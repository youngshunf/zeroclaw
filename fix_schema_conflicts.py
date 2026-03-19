#!/usr/bin/env python3
"""Resolve all 28 conflicts in config/schema.rs"""

import re

with open('src/config/schema.rs', 'r') as f:
    content = f.read()

# Track how many conflicts we resolve
count = 0

def resolve_conflict(match):
    global count
    count += 1
    head = match.group(1)
    upstream = match.group(2)
    
    head_stripped = head.strip()
    upstream_stripped = upstream.strip()
    
    # If HEAD is empty, take upstream (new fields/modules added by upstream)
    if not head_stripped:
        return upstream
    
    # If upstream is empty, take HEAD
    if not upstream_stripped:
        return head
    
    # Both non-empty - need merge logic
    return None  # Will handle manually

# First pass: resolve simple conflicts (one side empty)
pattern = re.compile(
    r'<<<<<<< HEAD\n(.*?)=======\n(.*?)>>>>>>> upstream/master\n',
    re.DOTALL
)

def simple_resolve(match):
    global count
    head = match.group(1)
    upstream = match.group(2)
    
    head_stripped = head.strip()
    upstream_stripped = upstream.strip()
    
    # HEAD empty → take upstream
    if not head_stripped:
        count += 1
        return upstream
    
    # Upstream empty → take HEAD  
    if not upstream_stripped:
        count += 1
        return head
    
    # Both non-empty → keep conflict marker for manual handling
    return match.group(0)

content = pattern.sub(simple_resolve, content)
print(f"Pass 1 (simple): resolved {count} conflicts")

# Pass 2: Handle specific patterns

# Pattern: comment-only changes (doc comments slightly different)
# e.g. "/// API key used for transcription requests." vs "/// API key used for transcription requests (Groq provider)."
# Take upstream (more specific)
def comment_resolve(match):
    global count
    head = match.group(1)
    upstream = match.group(2)
    
    head_lines = [l.strip() for l in head.strip().split('\n')]
    upstream_lines = [l.strip() for l in upstream.strip().split('\n')]
    
    # If both are just doc comments (///), take upstream
    if all(l.startswith('///') or l == '' for l in head_lines) and \
       all(l.startswith('///') or l == '' for l in upstream_lines):
        count += 1
        return upstream
    
    return match.group(0)

content = pattern.sub(comment_resolve, content)
remaining = len(pattern.findall(content))
print(f"Pass 2 (comments): total resolved {count}, remaining {remaining}")

# Pass 3: Handle ChannelsConfig struct fields - merge both sides
# Pattern: HEAD has wecom/ack_reaction, upstream has twitter/mochat/reddit/bluesky
def channels_struct_resolve(match):
    global count
    head = match.group(1)
    upstream = match.group(2)
    
    # ChannelsConfig struct field conflict: merge both
    if 'wecom' in head and ('twitter' in upstream or 'reddit' in upstream or 'bluesky' in upstream):
        # Keep HEAD's wecom + ack_reaction, add upstream's new channels
        # But need to handle carefully
        merged = head.rstrip()
        # Add upstream-only fields
        for line in upstream.strip().split('\n'):
            line_stripped = line.strip()
            # Add twitter, mochat, reddit, bluesky if not in HEAD
            if any(kw in line_stripped for kw in ['twitter', 'mochat', 'reddit', 'bluesky']):
                merged += '\n' + line
            # cfg(feature) for nostr
            if '#[cfg(feature' in line_stripped and 'nostr' in line_stripped:
                # upstream uses cfg(feature), HEAD doesn't - take upstream's conditional
                pass
        count += 1
        return merged + '\n'
    
    return match.group(0)

content = pattern.sub(channels_struct_resolve, content)
remaining = len(pattern.findall(content))
print(f"Pass 3 (channels struct): total resolved {count}, remaining {remaining}")

# Pass 4: Handle ChannelsConfig Default impl and test initializations
# These have the pattern: HEAD has show_tool_calls/ack_reaction/wecom, upstream has reddit/bluesky/session fields
def channels_default_resolve(match):
    global count
    head = match.group(1)
    upstream = match.group(2)
    
    head_stripped = head.strip()
    upstream_stripped = upstream.strip()
    
    # Default/test init: merge both sides' fields
    if ('show_tool_calls' in head or 'ack_reaction' in head or 'wecom' in head) and \
       ('reddit' in upstream or 'bluesky' in upstream or 'session_persistence' in upstream):
        
        lines = []
        # Take all HEAD lines
        for line in head.rstrip('\n').split('\n'):
            ls = line.strip()
            # Skip nostr line if upstream has cfg(feature) version
            if ls == 'nostr: None,' and '#[cfg(feature' in upstream:
                # Add cfg version instead
                lines.append('            #[cfg(feature = "channel-nostr")]')
                lines.append(line)
            else:
                lines.append(line)
        
        # Add upstream-only fields
        for line in upstream.strip().split('\n'):
            ls = line.strip()
            if any(kw in ls for kw in ['twitter:', 'mochat:', 'reddit:', 'bluesky:', 
                                         'session_persistence:', 'session_backend:', 'session_ttl_hours:',
                                         'ack_reactions:']):
                lines.append(line)
            if '#[cfg(feature' in ls and 'nostr' in ls:
                pass  # Already handled above
        
        count += 1
        return '\n'.join(lines) + '\n'
    
    return match.group(0)

content = pattern.sub(channels_default_resolve, content)
remaining = len(pattern.findall(content))
print(f"Pass 4 (channels default): total resolved {count}, remaining {remaining}")

# Pass 5: Handle Slack group_reply vs interrupt/mention_only
# Keep HEAD's group_reply AND add upstream's interrupt_on_new_message + mention_only
def slack_resolve(match):
    global count
    head = match.group(1)
    upstream = match.group(2)
    
    if 'group_reply' in head and ('interrupt_on_new_message' in upstream or 'mention_only' in upstream):
        # Keep both: HEAD's group_reply + upstream's new fields
        merged = head.rstrip('\n')
        for line in upstream.strip().split('\n'):
            ls = line.strip()
            if 'interrupt_on_new_message' in ls or 'mention_only' in ls:
                merged += '\n' + line
        count += 1
        return merged + '\n'
    
    return match.group(0)

content = pattern.sub(slack_resolve, content)
remaining = len(pattern.findall(content))
print(f"Pass 5 (slack): total resolved {count}, remaining {remaining}")

# Pass 6: Handle encrypt/decrypt channel secrets
# HEAD uses helper function decrypt_channel_secrets, upstream inlines individual decryptions
# Keep HEAD's helper function approach (cleaner) + add upstream's new TTS/STT decryptions
def crypto_resolve(match):
    global count
    head = match.group(1)
    upstream = match.group(2)
    
    if 'decrypt_channel_secrets' in head or 'encrypt_channel_secrets' in head:
        # Keep HEAD's helper + add upstream's TTS/STT decryptions
        merged = head.rstrip('\n') + '\n\n'
        # Extract TTS and STT blocks from upstream
        in_tts_stt = False
        tts_stt_lines = []
        for line in upstream.strip().split('\n'):
            if '// Decrypt TTS' in line or '// Encrypt TTS' in line or \
               '// Decrypt nested STT' in line or '// Encrypt nested STT' in line:
                in_tts_stt = True
            if in_tts_stt:
                tts_stt_lines.append(line)
            # Also grab decrypt_channel_secrets from upstream if present
            if 'decrypt_channel_secrets' in line or 'encrypt_channel_secrets' in line:
                pass  # HEAD already has this
        
        if tts_stt_lines:
            merged += '\n'.join(tts_stt_lines) + '\n'
        
        count += 1
        return merged
    
    return match.group(0)

content = pattern.sub(crypto_resolve, content)
remaining = len(pattern.findall(content))
print(f"Pass 6 (crypto): total resolved {count}, remaining {remaining}")

# Pass 7: Handle Config::default() - merge workspace/swarms + knowledge/linkedin
def config_default_resolve(match):
    global count
    head = match.group(1)
    upstream = match.group(2)
    
    if ('workspace' in head or 'swarms' in head) and ('knowledge' in upstream or 'linkedin' in upstream):
        # Keep both
        merged = head.rstrip('\n') + '\n'
        for line in upstream.strip().split('\n'):
            ls = line.strip()
            if 'knowledge' in ls or 'linkedin' in ls:
                merged += line + '\n'
        count += 1
        return merged
    
    return match.group(0)

content = pattern.sub(config_default_resolve, content)
remaining = len(pattern.findall(content))
print(f"Pass 7 (config default): total resolved {count}, remaining {remaining}")

# Pass 8: Handle test Config init - HEAD has ..Default::default(), upstream lists explicit fields
# Need to merge: keep HEAD's extra fields + upstream's new fields, use explicit listing
def test_config_resolve(match):
    global count
    head = match.group(1)
    upstream = match.group(2)
    
    if '..Default::default()' in head and ('knowledge' in upstream or 'linkedin' in upstream):
        # Replace ..Default::default() with explicit fields from upstream + HEAD's extra fields
        lines = []
        for line in head.rstrip('\n').split('\n'):
            ls = line.strip()
            if ls == '..Default::default()':
                # Add upstream's explicit fields instead
                for uline in upstream.strip().split('\n'):
                    lines.append(uline)
            else:
                lines.append(line)
        count += 1
        return '\n'.join(lines) + '\n'
    
    return match.group(0)

content = pattern.sub(test_config_resolve, content)
remaining = len(pattern.findall(content))
print(f"Pass 8 (test config): total resolved {count}, remaining {remaining}")

# Pass 9: Handle Slack test assertions
# HEAD: assert group_reply mode, upstream: assert interrupt/mention_only
def slack_test_resolve(match):
    global count
    head = match.group(1)
    upstream = match.group(2)
    
    if 'effective_group_reply_mode' in head and ('interrupt_on_new_message' in upstream or 'mention_only' in upstream):
        # Keep both assertions
        merged = head.rstrip('\n') + '\n'
        for line in upstream.strip().split('\n'):
            ls = line.strip()
            if 'interrupt_on_new_message' in ls or 'mention_only' in ls:
                merged += line + '\n'
        count += 1
        return merged
    
    # Upstream adds new test functions
    if not head.strip() and ('slack_config_deserializes_with_mention_only' in upstream or 
                              'slack_config_deserializes_interrupt' in upstream):
        count += 1
        return upstream
    
    return match.group(0)

content = pattern.sub(slack_test_resolve, content)
remaining = len(pattern.findall(content))
print(f"Pass 9 (slack tests): total resolved {count}, remaining {remaining}")

# Pass 10: Handle heartbeat two_phase vs ..default()
def heartbeat_resolve(match):
    global count
    head = match.group(1)
    upstream = match.group(2)
    
    if 'two_phase' in head and 'HeartbeatConfig::default()' in upstream:
        # Take upstream's cleaner ..default() pattern
        count += 1
        return upstream
    
    return match.group(0)

content = pattern.sub(heartbeat_resolve, content)
remaining = len(pattern.findall(content))
print(f"Pass 10 (heartbeat): total resolved {count}, remaining {remaining}")

# Pass 11: Handle the ChannelsConfig struct definition (line ~5362)
# This is the big one with wecom/ack_reaction vs twitter/mochat/reddit/bluesky
def channels_config_struct_resolve(match):
    global count
    head = match.group(1)
    upstream = match.group(2)
    
    # The struct definition conflict
    if 'ack_reaction' in head and 'ClawdTalkConfig' in upstream:
        # Build merged version: keep HEAD's wecom + ack_reaction, add upstream's new channels
        lines = []
        for line in head.rstrip('\n').split('\n'):
            ls = line.strip()
            # Replace ClawdTalk path if needed
            if 'crate::channels::clawdtalk::ClawdTalkConfig' in ls:
                line = line.replace('crate::channels::clawdtalk::ClawdTalkConfig', 'crate::channels::ClawdTalkConfig')
            lines.append(line)
        
        # Add upstream-only fields before the ack_reaction line
        insert_before_ack = []
        for line in upstream.strip().split('\n'):
            ls = line.strip()
            if any(kw in ls for kw in ['twitter', 'mochat', 'reddit', 'bluesky']):
                insert_before_ack.append(line)
        
        # Find ack_reaction line and insert before it
        result = []
        for line in lines:
            if 'ack_reaction' in line:
                result.extend(insert_before_ack)
            result.append(line)
        
        count += 1
        return '\n'.join(result) + '\n'
    
    return match.group(0)

content = pattern.sub(channels_config_struct_resolve, content)
remaining = len(pattern.findall(content))
print(f"Pass 11 (channels config struct): total resolved {count}, remaining {remaining}")

# Pass 12: Handle the feishu test at end of file
def feishu_test_resolve(match):
    global count
    head = match.group(1)
    upstream = match.group(2)
    
    if 'load_or_init_decrypts_feishu' in upstream:
        # Take upstream (new test)
        count += 1
        return upstream
    
    return match.group(0)

content = pattern.sub(feishu_test_resolve, content)
remaining = len(pattern.findall(content))
print(f"Pass 12 (feishu test): total resolved {count}, remaining {remaining}")

# Pass 13: Handle discord assert with interrupt/mention
def discord_assert_resolve(match):
    global count
    head = match.group(1)
    upstream = match.group(2)
    
    # Simple assertion additions
    if not head.strip() and ('interrupt_on_new_message' in upstream or 'mention_only' in upstream):
        count += 1
        return head + upstream
    
    return match.group(0)

content = pattern.sub(discord_assert_resolve, content)
remaining = len(pattern.findall(content))
print(f"Pass 13 (discord assert): total resolved {count}, remaining {remaining}")

with open('src/config/schema.rs', 'w') as f:
    f.write(content)

print(f"\nDone! Total resolved: {count}, Remaining: {remaining}")
