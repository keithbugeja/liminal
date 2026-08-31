use crate::core::channel::Subscriber;
use crate::core::message::Message;

use std::collections::{HashMap, HashSet};

pub const DEFAULT_IDLE_SLEEP_MS: u64 = 10;
pub const DEFAULT_MAX_DRAIN_PER_INPUT: usize = 64;

#[derive(Debug, Clone)]
pub struct InputMessage {
    pub input_name: String,
    pub message: Message,
}

pub async fn idle_sleep() {
    tokio::time::sleep(tokio::time::Duration::from_millis(DEFAULT_IDLE_SLEEP_MS)).await;
}

pub async fn try_one_each(inputs: &mut HashMap<String, Subscriber<Message>>) -> Vec<InputMessage> {
    let input_names = sorted_input_names(inputs);
    let mut messages = Vec::new();

    for input_name in input_names {
        let Some(input) = inputs.get_mut(&input_name) else {
            continue;
        };
        if let Some(message) = input.try_recv().await {
            messages.push(InputMessage {
                input_name,
                message,
            });
        }
    }

    messages
}

pub async fn drain_bounded_each(
    inputs: &mut HashMap<String, Subscriber<Message>>,
    max_per_input: usize,
) -> Vec<InputMessage> {
    if max_per_input == 0 {
        return Vec::new();
    }

    let input_names = sorted_input_names(inputs);
    let mut exhausted = HashSet::new();
    let mut drained_counts = HashMap::<String, usize>::new();
    let mut messages = Vec::new();

    while exhausted.len() < input_names.len() {
        let mut made_progress = false;

        for input_name in &input_names {
            if exhausted.contains(input_name) {
                continue;
            }

            let drained_count = drained_counts.entry(input_name.clone()).or_default();
            if *drained_count >= max_per_input {
                exhausted.insert(input_name.clone());
                continue;
            }

            let Some(input) = inputs.get_mut(input_name) else {
                exhausted.insert(input_name.clone());
                continue;
            };

            match input.try_recv().await {
                Some(message) => {
                    *drained_count += 1;
                    made_progress = true;
                    messages.push(InputMessage {
                        input_name: input_name.clone(),
                        message,
                    });
                }
                None => {
                    exhausted.insert(input_name.clone());
                }
            }
        }

        if !made_progress {
            break;
        }
    }

    messages
}

pub async fn latest_each(
    inputs: &mut HashMap<String, Subscriber<Message>>,
    max_per_input: usize,
) -> Vec<InputMessage> {
    if max_per_input == 0 {
        return Vec::new();
    }

    let input_names = sorted_input_names(inputs);
    let mut messages = Vec::new();

    for input_name in input_names {
        let Some(input) = inputs.get_mut(&input_name) else {
            continue;
        };

        let mut latest_message = None;
        for _ in 0..max_per_input {
            let Some(message) = input.try_recv().await else {
                break;
            };
            latest_message = Some(message);
        }

        if let Some(message) = latest_message {
            messages.push(InputMessage {
                input_name,
                message,
            });
        }
    }

    messages
}

fn sorted_input_names(inputs: &HashMap<String, Subscriber<Message>>) -> Vec<String> {
    let mut input_names = inputs.keys().cloned().collect::<Vec<_>>();
    input_names.sort();
    input_names
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::channel::Subscriber;
    use serde_json::json;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn try_one_each_reads_once_from_each_input() {
        let mut inputs = HashMap::new();
        let (a_tx, a_rx) = mpsc::channel(4);
        let (b_tx, b_rx) = mpsc::channel(4);

        inputs.insert("b".to_string(), Subscriber::Mpsc(b_rx));
        inputs.insert("a".to_string(), Subscriber::Mpsc(a_rx));

        a_tx.send(Message::new("a", "a", json!(1))).await.unwrap();
        a_tx.send(Message::new("a", "a", json!(2))).await.unwrap();
        b_tx.send(Message::new("b", "b", json!(10))).await.unwrap();

        let messages = try_one_each(&mut inputs).await;

        assert_eq!(
            messages
                .iter()
                .map(|message| message.input_name.as_str())
                .collect::<Vec<_>>(),
            vec!["a", "b"]
        );
    }

    #[tokio::test]
    async fn drain_bounded_each_interleaves_inputs_by_round() {
        let mut inputs = HashMap::new();
        let (a_tx, a_rx) = mpsc::channel(8);
        let (b_tx, b_rx) = mpsc::channel(8);

        inputs.insert("b".to_string(), Subscriber::Mpsc(b_rx));
        inputs.insert("a".to_string(), Subscriber::Mpsc(a_rx));

        for value in 0..3 {
            a_tx.send(Message::new("a", "a", json!(value)))
                .await
                .unwrap();
            b_tx.send(Message::new("b", "b", json!(value)))
                .await
                .unwrap();
        }

        let messages = drain_bounded_each(&mut inputs, 2).await;

        assert_eq!(
            messages
                .iter()
                .map(|message| message.input_name.as_str())
                .collect::<Vec<_>>(),
            vec!["a", "b", "a", "b"]
        );
    }

    #[tokio::test]
    async fn latest_each_keeps_only_newest_message_per_input() {
        let mut inputs = HashMap::new();
        let (a_tx, a_rx) = mpsc::channel(8);

        inputs.insert("a".to_string(), Subscriber::Mpsc(a_rx));

        for value in 0..3 {
            a_tx.send(Message::new("a", "a", json!(value)))
                .await
                .unwrap();
        }

        let messages = latest_each(&mut inputs, 64).await;

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].message.payload, json!(2));
    }
}
