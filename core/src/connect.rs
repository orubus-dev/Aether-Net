use crate::network::NodeId;
use crate::number::NumberId;
use crate::storage::ContentId;
use sha2::{Digest, Sha256};

pub const MAX_GROUP_MEMBERS: usize = 1_024;
pub const MAX_OFFLINE_MESSAGES: usize = 16_384;
pub const MAX_MESSAGE_BYTES: usize = 1 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConversationId([u8; 32]);
impl ConversationId { pub fn derive(a:NodeId,b:NodeId)->Self{let (x,y)=if a.as_bytes()<=b.as_bytes(){(a,b)}else{(b,a)};let mut h=Sha256::new();h.update(b"aether:conversation:v1:");h.update(x.as_bytes());h.update(y.as_bytes());Self(h.finalize().into())} }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presence { Offline, Online, Away, Busy, Calling }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKind { Text, File, Image, Audio, Video, VoiceCall, VideoCall }
#[derive(Debug, Clone)]
pub struct MessageEnvelope { pub conversation:ConversationId,pub sender:NodeId,pub sequence:u64,pub media:MediaKind,pub content:ContentId,pub sent_at:u64 }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Group { pub id:ConversationId,pub owner:NodeId,pub version:u64 }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OfflineMessage { pub target:NodeId,pub message:MessageEnvelope,pub expires_at:u64 }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectError { GroupCapacityExceeded, MemberCapacityExceeded, OfflineQueueFull, MessageTooLarge, InvalidMessage, OfflineOnly }

pub struct AetherConnect { max_members:usize,max_offline:usize,groups:Vec<(Group,Vec<NodeId>)>,offline:Vec<OfflineMessage> }
impl AetherConnect{
 pub fn new()->Self{Self{max_members:MAX_GROUP_MEMBERS,max_offline:MAX_OFFLINE_MESSAGES,groups:Vec::new(),offline:Vec::new()}}
 pub fn create_group(&mut self,group:Group,members:Vec<NodeId>)->Result<bool,ConnectError>{if members.is_empty()||members.len()>self.max_members{return Err(ConnectError::MemberCapacityExceeded)}if self.groups.len()>=MAX_GROUP_MEMBERS{return Err(ConnectError::GroupCapacityExceeded)}self.groups.push((group,members));Ok(true)}
 pub fn enqueue_offline(&mut self,item:OfflineMessage,now:u64)->Result<bool,ConnectError>{if item.message.sender==item.target||item.message.sent_at>now||item.expires_at<=now{return Err(ConnectError::InvalidMessage)}if self.offline.len()>=self.max_offline{return Err(ConnectError::OfflineQueueFull)}self.offline.push(item);Ok(true)}
 pub fn drain_offline(&mut self,target:NodeId,now:u64)->Vec<MessageEnvelope>{let mut out=Vec::new();let mut kept=Vec::with_capacity(self.offline.len());for item in self.offline.drain(..){if item.expires_at<=now{continue}if item.target==target{out.push(item.message)}else{kept.push(item)}}self.offline=kept;out}
 pub fn offline_len(&self)->usize{self.offline.len()}
}
impl Default for AetherConnect{fn default()->Self{Self::new()}}
