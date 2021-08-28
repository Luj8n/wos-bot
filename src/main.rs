use dotenv;
use itertools::Itertools;
use std::{collections::HashMap, fs, time::Instant};
use twitch_irc::{
  login::StaticLoginCredentials, message::ServerMessage, ClientConfig, SecureTCPTransport,
  TwitchIRCClient,
};

fn get_env_var(name: &str) -> String {
  return dotenv::var(name).expect(&format!("Couldn't find {} variable in the .env file", name));
}

fn get_word_difference(letters: &HashMap<u8, usize>, word: &HashMap<u8, usize>) -> usize {
  word.iter().fold(0, |acc, e| {
    if !letters.contains_key(e.0) {
      acc + e.1
    } else {
      if &letters[e.0] >= e.1 {
        acc
      } else {
        acc + e.1 - letters[e.0]
      }
    }
  })
}

fn get_possible_words(
  word: &str,
  min_count: usize,
  max_count: usize,
  bonus_letter_count: usize,
  word_list_choice: &str,
) -> Result<Vec<String>, std::io::Error> {
  let start_time = Instant::now();
  let text = fs::read_to_string(format!("./word-lists/{}.txt", word_list_choice))?;

  let word_list = text
    .split('\n')
    .filter(|w| w.trim() != "")
    .map(|w| w.to_lowercase());

  let letters = word.to_lowercase().bytes().counts();

  let selected = word_list
    .filter(|w| {
      (min_count..=max_count).contains(&w.len())
        && get_word_difference(&letters, &w.bytes().counts()) <= bonus_letter_count
    })
    .sorted()
    .sorted_by(|a, b| Ord::cmp(&a.len(), &b.len()))
    .collect_vec();

  let diff = start_time.elapsed().as_millis();
  println!("Time taken to guess: {:?}", diff);

  Ok(selected)
}

#[tokio::main]
pub async fn main() {
  let login_name = get_env_var("USERNAME");
  let oauth_token = get_env_var("OAUTH_TOKEN");
  let channels = get_env_var("CHANNELS")
    .split(',')
    .map(|channel| channel.to_string().to_lowercase())
    .collect_vec();
  let bot_prefix = get_env_var("BOT_PREFIX");

  let config = ClientConfig::new_simple(StaticLoginCredentials::new(
    login_name.to_owned(),
    Some(oauth_token),
  ));

  let (mut incoming_messages, client) =
    TwitchIRCClient::<SecureTCPTransport, StaticLoginCredentials>::new(config);

  for channel in channels {
    client.join(channel)
  }

  let join_handle = tokio::spawn(async move {
    while let Some(message) = incoming_messages.recv().await {
      match message {
        ServerMessage::Privmsg(msg) => {
          // println!(
          //   "#{} -> {}: {}",
          //   msg.channel_login, msg.sender.name, msg.message_text
          // );

          let sender_is_mod = msg
            .badges
            .iter()
            .any(|badge| ["moderator", "broadcaster"].contains(&&badge.name[..]));

          let words = msg
            .message_text
            .split_whitespace()
            .map(str::to_string)
            .collect_vec();

          if !msg.message_text.starts_with(&bot_prefix) || !sender_is_mod {
            continue;
          }

          if let Some(command) = words.get(0) {
            match &command.to_lowercase()[1..] {
              "guess" => {
                if let Some(word) = words.get(1) {
                  if let Some(Ok(min_count)) = words.get(2).map(|x| x.parse()) {
                    if let Some(Ok(max_count)) = words.get(3).map(|x| x.parse()) {
                      if let Some(Ok(bonus_letter_count)) = words.get(4).map(|x| x.parse()) {
                        if let Some(word_list_choice) = words.get(5) {
                          match get_possible_words(
                            word,
                            min_count,
                            max_count,
                            bonus_letter_count,
                            word_list_choice,
                          ) {
                            Ok(possible_words) => {
                              // for w in possible_words {
                              //   client.say(msg.channel_login.clone(), w).await.unwrap();
                              //   thread::sleep(time::Duration::from_millis(250));
                              // }

                              let mut char_count = 0;
                              let mut segments: Vec<Vec<String>> = vec![vec![]];

                              possible_words.iter().for_each(|x| {
                                char_count += x.len() + 1;

                                if char_count >= 500 {
                                  segments.push(vec![]);
                                  char_count -= 500;
                                }

                                segments
                                  .last_mut()
                                  .expect("Vec is for some reason empty?")
                                  .push(x.to_owned());
                              });

                              for segment in segments {
                                client
                                  .say(msg.channel_login.clone(), segment.join(" "))
                                  .await
                                  .expect("Error sending message");
                              }
                            }
                            Err(error) => {
                              println!("{:?}", error);
                            }
                          }
                        }
                      }
                    }
                  }
                }
              }
              _ => {}
            }
          }
        }
        _ => {}
      }
    }
  });

  join_handle.await.unwrap();
}
