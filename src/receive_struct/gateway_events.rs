use serde_derive::Deserialize;
use serde_json::Value;

use crate::utils::SnowflakeID;

#[derive(Deserialize, Debug)]
pub struct ResumedData {
    #[serde(default, rename = "_trace")]
    pub trace: Vec<Value>,
}

#[derive(Deserialize, Debug)]
pub struct ReadySupplementalData {
    #[serde(default)]
    pub guilds: Vec<Value>,
    #[serde(default)]
    pub merged_members: Vec<Value>,
    #[serde(default)]
    pub merged_presences: Option<Value>,
}

#[derive(Deserialize, Debug)]
pub struct PassiveUpdateV1Data {
    pub guild_id: SnowflakeID,
    #[serde(default)]
    pub channels: Vec<Value>,
    #[serde(default)]
    pub members: Vec<Value>,
    #[serde(default)]
    pub voice_states: Vec<Value>,
}

#[derive(Deserialize, Debug)]
pub struct MessageReactionAdd {
    pub user_id: SnowflakeID,
    pub message_id: SnowflakeID,
    pub channel_id: SnowflakeID,
    pub guild_id: Option<SnowflakeID>,
    pub emoji: ReactionEmoji,
    #[serde(default)]
    pub burst: bool,
    pub message_author_id: Option<SnowflakeID>,
}

#[derive(Deserialize, Debug)]
pub struct MessageReactionRemove {
    pub user_id: SnowflakeID,
    pub message_id: SnowflakeID,
    pub channel_id: SnowflakeID,
    pub guild_id: Option<SnowflakeID>,
    pub emoji: ReactionEmoji,
    #[serde(default)]
    pub burst: bool,
}

#[derive(Deserialize, Debug)]
pub struct MessageReactionRemoveAll {
    pub message_id: SnowflakeID,
    pub channel_id: SnowflakeID,
    pub guild_id: Option<SnowflakeID>,
}

#[derive(Deserialize, Debug)]
pub struct MessageReactionRemoveEmoji {
    pub message_id: SnowflakeID,
    pub channel_id: SnowflakeID,
    pub guild_id: Option<SnowflakeID>,
    pub emoji: ReactionEmoji,
}

#[derive(Deserialize, Debug)]
pub struct ReactionEmoji {
    pub id: Option<String>,
    pub name: Option<String>,
    #[serde(default)]
    pub animated: bool,
}

#[derive(Deserialize, Debug)]
pub struct MessageDeleteBulk {
    #[serde(default)]
    pub ids: Vec<SnowflakeID>,
    pub channel_id: SnowflakeID,
    pub guild_id: Option<SnowflakeID>,
}

#[derive(Deserialize, Debug)]
pub struct MessagePollVote {
    pub user_id: SnowflakeID,
    pub channel_id: SnowflakeID,
    pub message_id: SnowflakeID,
    pub guild_id: Option<SnowflakeID>,
    pub answer_id: i64,
}

#[derive(Deserialize, Debug)]
pub struct GuildCreate {
    pub id: SnowflakeID,
    pub name: Option<String>,
    pub properties: Option<crate::receive_struct::ready::GuildProperties>,
    pub member_count: Option<i64>,
    #[serde(default)]
    pub unavailable: bool,
}

#[derive(Deserialize, Debug)]
pub struct GuildUpdate {
    pub id: SnowflakeID,
    pub name: Option<String>,
    pub properties: Option<crate::receive_struct::ready::GuildProperties>,
}

#[derive(Deserialize, Debug)]
pub struct GuildBanAdd {
    pub guild_id: SnowflakeID,
    pub user: UserStub,
}

#[derive(Deserialize, Debug, Default, Clone)]
pub struct UserStub {
    pub id: SnowflakeID,
    pub username: Option<String>,
    pub global_name: Option<String>,
    pub discriminator: Option<String>,
    #[serde(default)]
    pub bot: bool,
    pub avatar: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct GuildRoleUpsert {
    pub guild_id: SnowflakeID,
    pub role: Role,
}

#[derive(Deserialize, Debug)]
pub struct Role {
    pub id: SnowflakeID,
    pub name: Option<String>,
    pub color: Option<i64>,
    #[serde(default)]
    pub managed: bool,
    #[serde(default)]
    pub mentionable: bool,
    pub permissions: Option<String>,
    pub position: Option<i64>,
}

#[derive(Deserialize, Debug)]
pub struct GuildRoleDelete {
    pub guild_id: SnowflakeID,
    pub role_id: SnowflakeID,
}

#[derive(Deserialize, Debug)]
pub struct GuildEmojisUpdate {
    pub guild_id: SnowflakeID,
    #[serde(default)]
    pub emojis: Vec<Value>,
}

#[derive(Deserialize, Debug)]
pub struct GuildStickersUpdate {
    pub guild_id: SnowflakeID,
    #[serde(default)]
    pub stickers: Vec<Value>,
}

#[derive(Deserialize, Debug)]
pub struct GuildApplicationCommandIndexUpdate {
    pub guild_id: SnowflakeID,
    #[serde(default)]
    pub application_command_counts: serde_json::Map<String, Value>,
}

#[derive(Deserialize, Debug)]
pub struct GuildAuditLogEntryCreate {
    pub id: SnowflakeID,
    pub guild_id: SnowflakeID,
    pub user_id: Option<SnowflakeID>,
    pub target_id: Option<String>,
    pub action_type: i64,
    pub reason: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct GuildMemberAdd {
    pub guild_id: SnowflakeID,
    pub user: UserStub,
    pub nick: Option<String>,
    pub joined_at: Option<String>,
    pub premium_since: Option<String>,
    #[serde(default)]
    pub roles: Vec<String>,
    pub pending: Option<bool>,
}

#[derive(Deserialize, Debug)]
pub struct GuildMemberUpdate {
    pub guild_id: SnowflakeID,
    pub user: UserStub,
    pub nick: Option<String>,
    pub avatar: Option<String>,
    pub joined_at: Option<String>,
    pub premium_since: Option<String>,
    #[serde(default)]
    pub roles: Vec<String>,
    pub pending: Option<bool>,
    pub communication_disabled_until: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct GuildMemberChunk {
    pub guild_id: SnowflakeID,
    pub chunk_index: Option<i64>,
    pub chunk_count: Option<i64>,
    #[serde(default)]
    pub members: Vec<Value>,
    #[serde(default)]
    pub not_found: Vec<Value>,
    #[serde(default)]
    pub presences: Vec<Value>,
    pub nonce: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct ThreadEvent {
    pub id: SnowflakeID,
    pub guild_id: Option<SnowflakeID>,
    pub parent_id: Option<SnowflakeID>,
    pub owner_id: Option<SnowflakeID>,
    pub name: Option<String>,
    #[serde(rename = "type", default)]
    pub type_field: i64,
    pub message_count: Option<i64>,
    pub member_count: Option<i64>,
    pub rate_limit_per_user: Option<i64>,
    pub last_message_id: Option<SnowflakeID>,
    pub thread_metadata: Option<Value>,
    pub member: Option<Value>,
    #[serde(default)]
    pub applied_tags: Vec<String>,
}

#[derive(Deserialize, Debug)]
pub struct ThreadListSync {
    pub guild_id: SnowflakeID,
    #[serde(default)]
    pub channel_ids: Vec<SnowflakeID>,
    #[serde(default)]
    pub threads: Vec<Value>,
    #[serde(default)]
    pub members: Vec<Value>,
}

#[derive(Deserialize, Debug)]
pub struct ThreadMembersUpdate {
    pub id: SnowflakeID,
    pub guild_id: Option<SnowflakeID>,
    pub member_count: Option<i64>,
    #[serde(default)]
    pub added_members: Vec<Value>,
    #[serde(default)]
    pub removed_member_ids: Vec<SnowflakeID>,
}

#[derive(Deserialize, Debug)]
pub struct InviteCreate {
    pub channel_id: SnowflakeID,
    pub guild_id: Option<SnowflakeID>,
    pub code: String,
    pub created_at: Option<String>,
    pub inviter: Option<UserStub>,
    pub max_age: Option<i64>,
    pub max_uses: Option<i64>,
    #[serde(default)]
    pub temporary: bool,
    pub uses: Option<i64>,
    pub target_type: Option<i64>,
    pub target_user: Option<UserStub>,
}

#[derive(Deserialize, Debug)]
pub struct InviteDelete {
    pub channel_id: SnowflakeID,
    pub guild_id: Option<SnowflakeID>,
    pub code: String,
}

#[derive(Deserialize, Debug)]
pub struct WebhooksUpdate {
    pub guild_id: SnowflakeID,
    pub channel_id: SnowflakeID,
}

#[derive(Deserialize, Debug)]
pub struct UserUpdate {
    pub id: SnowflakeID,
    pub username: Option<String>,
    pub global_name: Option<String>,
    pub discriminator: Option<String>,
    pub email: Option<String>,
    pub avatar: Option<String>,
    pub bio: Option<String>,
    #[serde(default)]
    pub mfa_enabled: bool,
    #[serde(default)]
    pub verified: bool,
    pub phone: Option<String>,
    pub flags: Option<i64>,
    pub premium_type: Option<i64>,
}

#[derive(Deserialize, Debug)]
pub struct UserNoteUpdate {
    pub id: SnowflakeID,
    pub note: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct UserNonChannelAck {
    pub entity_id: Option<SnowflakeID>,
    pub ack_type: Option<i64>,
    pub version: Option<i64>,
}

#[derive(Deserialize, Debug)]
pub struct UserApplicationUpdate {
    pub application_id: SnowflakeID,
}

#[derive(Deserialize, Debug)]
pub struct UserConnectionsUpdate {
    #[serde(default)]
    pub connections: Vec<Value>,
}

#[derive(Deserialize, Debug)]
pub struct UserSettingsProtoUpdate {
    pub settings: Option<Value>,
    #[serde(default)]
    pub partial: bool,
}

#[derive(Deserialize, Debug)]
pub struct RelationshipAdd {
    pub id: SnowflakeID,

    #[serde(rename = "type")]
    pub type_field: i64,
    pub user: Option<UserStub>,
    pub nickname: Option<String>,
    pub since: Option<String>,
    #[serde(default)]
    pub should_notify: bool,
}

#[derive(Deserialize, Debug)]
pub struct RelationshipRemove {
    pub id: SnowflakeID,
    #[serde(rename = "type")]
    pub type_field: Option<i64>,
    pub nickname: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct InteractionCreate {
    pub id: SnowflakeID,
    pub nonce: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct InteractionSuccess {
    pub id: SnowflakeID,
    pub nonce: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct GuildScheduledEvent {
    pub id: SnowflakeID,
    pub guild_id: SnowflakeID,
    pub channel_id: Option<SnowflakeID>,
    pub creator_id: Option<SnowflakeID>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub scheduled_start_time: Option<String>,
    pub scheduled_end_time: Option<String>,
    #[serde(default)]
    pub privacy_level: i64,
    pub status: Option<i64>,
    pub entity_type: Option<i64>,
    pub entity_id: Option<SnowflakeID>,
    pub entity_metadata: Option<Value>,
    pub creator: Option<Value>,
    pub user_count: Option<i64>,
    pub image: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct GuildScheduledEventUser {
    pub guild_scheduled_event_id: SnowflakeID,
    pub user_id: SnowflakeID,
    pub guild_id: SnowflakeID,
}

#[derive(Deserialize, Debug)]
pub struct GuildSoundboardSound {
    pub sound_id: SnowflakeID,
    pub guild_id: Option<SnowflakeID>,
    pub name: Option<String>,
    pub volume: Option<f64>,
    pub emoji_id: Option<String>,
    pub emoji_name: Option<String>,
    pub user_id: Option<SnowflakeID>,
    pub available: Option<bool>,
}

#[derive(Deserialize, Debug)]
pub struct GuildSoundboardSoundsUpdate {
    pub guild_id: SnowflakeID,
    #[serde(default)]
    pub soundboard_sounds: Vec<Value>,
}

#[derive(Deserialize, Debug)]
pub struct SoundboardSounds {
    pub guild_id: SnowflakeID,
    #[serde(default)]
    pub soundboard_sounds: Vec<Value>,
}

#[derive(Deserialize, Debug)]
pub struct StageInstance {
    pub id: SnowflakeID,
    pub guild_id: SnowflakeID,
    pub channel_id: SnowflakeID,
    pub topic: Option<String>,
    pub privacy_level: Option<i64>,
    pub discoverable_disabled: Option<bool>,
}

#[derive(Deserialize, Debug)]
pub struct Entitlement {
    pub id: SnowflakeID,
    pub sku_id: Option<SnowflakeID>,
    pub user_id: Option<SnowflakeID>,
    pub guild_id: Option<SnowflakeID>,
    pub application_id: Option<SnowflakeID>,
    #[serde(rename = "type")]
    pub type_field: Option<i64>,
    pub deleted: Option<bool>,
    pub starts_at: Option<String>,
    pub ends_at: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct Subscription {
    pub id: SnowflakeID,
    pub user_id: Option<SnowflakeID>,
    pub status: Option<i64>,
    pub current_period_start: Option<String>,
    pub current_period_end: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct GuildJoinRequest {
    pub guild_id: SnowflakeID,
    pub status: Option<String>,
    pub request: Option<Value>,
}

#[derive(Deserialize, Debug)]
pub struct AutoModerationRule {
    pub id: SnowflakeID,
    pub guild_id: SnowflakeID,
    pub name: Option<String>,
    pub creator_id: Option<SnowflakeID>,
    pub event_type: Option<i64>,
    pub trigger_type: Option<i64>,
    pub trigger_metadata: Option<Value>,
    #[serde(default)]
    pub actions: Vec<Value>,
    #[serde(default)]
    pub enabled: bool,
}

#[derive(Deserialize, Debug)]
pub struct AutoModerationExecution {
    pub guild_id: SnowflakeID,
    pub action: Option<Value>,
    pub rule_id: Option<SnowflakeID>,
    pub user_id: Option<SnowflakeID>,
    pub channel_id: Option<SnowflakeID>,
    pub message_id: Option<SnowflakeID>,
    pub matched_keyword: Option<String>,
    pub content: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct ApplicationCommandPermissionsUpdate {
    pub id: SnowflakeID,
    pub application_id: SnowflakeID,
    pub guild_id: SnowflakeID,
    #[serde(default)]
    pub permissions: Vec<Value>,
}

#[derive(Deserialize, Debug)]
pub struct ChannelPinsUpdate {
    pub channel_id: SnowflakeID,
    pub guild_id: Option<SnowflakeID>,
    pub last_pin_timestamp: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct ChannelPinsAck {
    pub channel_id: SnowflakeID,
    pub timestamp: Option<String>,
    pub version: Option<i64>,
}

#[derive(Deserialize, Debug)]
pub struct ChannelTopicUpdate {
    pub channel_id: SnowflakeID,
    pub guild_id: Option<SnowflakeID>,
    pub topic: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct VoiceChannelEffectSend {
    pub channel_id: SnowflakeID,
    pub guild_id: Option<SnowflakeID>,
    pub user_id: Option<SnowflakeID>,
    pub emoji: Option<ReactionEmoji>,
    pub animation_type: Option<i64>,
    pub animation_id: Option<i64>,
    pub sound_id: Option<SnowflakeID>,
    pub sound_volume: Option<f64>,
}

#[derive(Deserialize, Debug)]
pub struct IntegrationDelete {
    pub id: SnowflakeID,
    pub guild_id: SnowflakeID,
    pub application_id: Option<SnowflakeID>,
}

#[derive(Deserialize, Debug)]
pub struct EmbeddedActivityUpdateV2 {
    pub guild_id: Option<SnowflakeID>,
    pub instance_id: Option<String>,
    pub launch_id: Option<String>,
    pub application_id: Option<SnowflakeID>,
    pub location: Option<Value>,
    pub composite_instance_id: Option<String>,
    #[serde(default)]
    pub participants: Vec<Value>,
}

#[derive(Deserialize, Debug)]
pub struct ConversationSummaryUpdate {
    pub channel_id: SnowflakeID,
    pub guild_id: Option<SnowflakeID>,
    #[serde(default)]
    pub summaries: Vec<Value>,
}

#[derive(Deserialize, Debug)]
pub struct NotificationCenterItem {
    pub id: Option<String>,
    pub user_id: Option<SnowflakeID>,
    #[serde(rename = "type")]
    pub type_field: Option<String>,
    pub completed: Option<bool>,
    pub viewed: Option<bool>,
    pub other_user: Option<UserStub>,
    pub created_at: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct RecentMentionDelete {
    pub message_id: SnowflakeID,
}

#[derive(Deserialize, Debug)]
pub struct ContentInventoryInboxStale {
    pub user_id: Option<SnowflakeID>,
}

#[derive(Deserialize, Debug)]
pub struct Oauth2TokenCreate {
    pub id: SnowflakeID,
    #[serde(default)]
    pub scopes: Vec<String>,
    pub application: Option<Value>,
}
