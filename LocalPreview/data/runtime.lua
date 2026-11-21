-- This file is loaded as data, so appearance and game-logic updates need no rebuild.
-- No Lua tables belonging to a received packet are modified by this runtime.
if not MJLocalTrial then
  MJLocalTrial = { catalog = @CATALOG@, guard_ready = false, choices = {}, skins = {}, groups = {} }
  local M = MJLocalTrial
  local function copy(value)
    if type(value) ~= 'table' then return value end
    local out = {}
    -- Protocol messages keep their public fields behind descriptor getters.
    -- pairs(message) only exposes private bookkeeping, not the login payload.
    local meta = getmetatable(value)
    local fields = type(meta) == 'table' and meta._descriptor and meta._descriptor.fields
    if fields then
      for _, field in ipairs(fields) do out[field.name] = value[field.name] end
      if value.HasField then
        out.HasField = function(self, name)
          if self[name] ~= value[name] then return self[name] ~= nil end
          return value:HasField(name)
        end
      end
    else
      for k, v in pairs(value) do out[k] = v end
    end
    return out
  end
  local function array(value)
    local out = {}
    for i = 1, #(value or {}) do out[i] = value[i] end
    return out
  end
  local function config(section, id)
    if not ExcelMgr or not ExcelMgr.GetData or not id then return nil end
    local ok, data = pcall(ExcelMgr.GetData, 'item_definition', section, id)
    return ok and type(data) == 'table' and data or nil
  end
  local function has(section, id)
    if not id then return false end
    M.sets = M.sets or {}
    if not M.sets[section] then
      local set = {}
      for _, value in ipairs(M.catalog[section] or {}) do set[value] = true end
      M.sets[section] = set
    end
    return M.sets[section][id] == true
  end
  local function merge_ids(real, extra, section)
    local out, seen = array(real), {}
    for _, id in ipairs(out) do seen[id] = true end
    for _, id in ipairs(extra or {}) do
      if not seen[id] and config(section, id) then seen[id] = true; out[#out + 1] = id end
    end
    return out
  end
  local function valid_skin(charid, skin)
    local c, s = config('character', charid), config('skin', skin)
    return c and s and (not s.character_id or s.character_id == charid)
  end
  local function profile_key(id)
    if type(id) == 'number' and id > 0 then return 'MJLocalPreview.appearance.v1.' .. string.format('%.0f', id) end
  end
  -- Use Unity's local PlayerPrefs directly, with our own account-specific key.
  -- Only numeric IDs are parsed; saved data is never executed as Lua.
  function M.restore()
    local key = profile_key(M.account_id)
    local prefs = UnityEngine and UnityEngine.PlayerPrefs
    if not key or not prefs then return end
    local ok, text = pcall(prefs.GetString, key, '')
    if not ok or type(text) ~= 'string' or #text > 32768 then return end
    local main, skins = text:match('^1;(%d+);([%d:,]*)$')
    if not main then return end
    main = tonumber(main)
    if config('character', main) then M.choices.character_id = main end
    for entry in skins:gmatch('[^,]+') do
      local id, skin = entry:match('^(%d+):(%d+)$')
      id, skin = tonumber(id), tonumber(skin)
      if id and skin and valid_skin(id, skin) then M.skins[id] = skin end
    end
  end
  function M.save()
    local key = profile_key(M.account_id)
    local prefs = UnityEngine and UnityEngine.PlayerPrefs
    if not key or not prefs then return end
    local entries = {}
    for id, skin in pairs(M.skins) do
      if valid_skin(id, skin) then entries[#entries + 1] = string.format('%.0f:%.0f', id, skin) end
    end
    table.sort(entries)
    local text = '1;' .. string.format('%.0f', M.choices.character_id or 0) .. ';' .. table.concat(entries, ',')
    local ok = pcall(function() prefs.SetString(key, text); prefs.Save() end)
    if not ok then M.log('local appearance save failed') end
  end
  local function sort_key()
    if profile_key(M.account_id) then
      return 'MJLocalPreview.' .. 'characterSort.v1.' .. string.format('%.0f', M.account_id)
    end
  end
  local function character_ids(ids, seen)
    local out = {}
    seen = seen or {}
    for _, id in ipairs(ids or {}) do
      if type(id) == 'number' and id > 0 and id == math.floor(id) and
        config('character', id) and not seen[id] then
        seen[id] = true; out[#out + 1] = id
      end
    end
    return out
  end
  function M.restore_sort()
    local key, prefs = sort_key(), UnityEngine and UnityEngine.PlayerPrefs
    if not key or not prefs then return end
    local ok, text = pcall(prefs.GetString, key, '')
    if not ok or type(text) ~= 'string' or #text > 32768 then return end
    local favorites, others, hidden = text:match('^1;([%d,]*);([%d,]*);([%d,]*)$')
    if not favorites then return end
    local function parse(value)
      local ids = {}
      for id in value:gmatch('%d+') do ids[#ids + 1] = tonumber(id) end
      return ids
    end
    local seen = {}
    M.character_sort = character_ids(parse(favorites), seen)
    M.other_character_sort = character_ids(parse(others), seen)
    M.hidden_characters = character_ids(parse(hidden))
  end
  function M.save_sort(data)
    -- Empty lists are intentional: removing the final star must survive a restart.
    local seen = {}
    M.character_sort = character_ids(data.sort, seen)
    M.other_character_sort = character_ids(data.other_sort, seen)
    M.hidden_characters = character_ids(data.hidden_characters)
    local key, prefs = sort_key(), UnityEngine and UnityEngine.PlayerPrefs
    if not key or not prefs then return end
    local text = '1;' .. table.concat(M.character_sort, ',') .. ';' ..
      table.concat(M.other_character_sort, ',') .. ';' .. table.concat(M.hidden_characters, ',')
    local ok = pcall(function() prefs.SetString(key, text); prefs.Save() end)
    if not ok then M.log('local character sort save failed') end
  end
  function M.account(mgr)
    if not mgr then return end
    local id = mgr.account_id
    if M.account_id ~= id then
      M.account_id = id
      M.choices, M.skins, M.groups, M.character_views = {}, {}, {}, {}
      M.match_views = nil
      M.active_group, M.last_info, M.loading_images, M.title = nil, nil, nil, nil
      M.real_characters = {}
      M.character_sort, M.other_character_sort, M.hidden_characters = nil, nil, nil
      M.restore()
      M.restore_sort()
    end
  end
  function M.log(stage)
    if M.logged and M.logged[stage] then return end
    M.logged = M.logged or {}; M.logged[stage] = true
    if _MJLocalPreviewTrace then _MJLocalPreviewTrace(stage) end
    pcall(function() UnityEngine.Debug.Log('[MJLocalPreview] ' .. stage) end)
  end
  function M.owned(id)
    if not M.guard_ready then return false end
    if has('character', id) then return config('character', id) ~= nil end
    if has('skin', id) then return config('skin', id) ~= nil end
    if has('title', id) then return config('title', id) ~= nil end
    if has('item', id) or has('loading_image', id) then
      local item = config('item', id)
      return item ~= nil and (item.category == 4 or item.category == 5 or item.category == 8)
    end
    return false
  end
  function M.bag_count(id)
    if not M.guard_ready or not (has('item', id) or has('loading_image', id)) then return nil end
    local item = config('item', id)
    if not item or (item.category ~= 5 and item.category ~= 8) then return nil end
    local real = UI_Bag and UI_Bag.Inst and UI_Bag.Inst.getItem and UI_Bag.Inst.getItem[id]
    return math.max(real and real.stack or 0, 1)
  end
  function M.bag_items(category, kind, excluded)
    if not M.guard_ready or (category ~= 5 and category ~= 8) then return nil end
    local out, seen, skip = {}, {}, {}
    for _, id in ipairs(excluded or {}) do skip[id] = true end
    local function add(entry)
      local item = entry.data or config('item', entry.item_id)
      if item and item.category == category and (kind == nil or item.type == kind) and
        not skip[item.type] and entry.stack > 0 and not seen[entry.item_id] then
        out[#out + 1], seen[entry.item_id] = entry, true
      end
    end
    for _, entry in pairs(UI_Bag and UI_Bag.Inst and UI_Bag.Inst.getItem or {}) do add(entry) end
    for _, section in ipairs({ 'item', 'loading_image' }) do
      for _, id in ipairs(M.catalog[section] or {}) do
        if not seen[id] then
          local item = config('item', id)
          if item then add({ item_id = id, data = item, stack = 1 }) end
        end
      end
    end
    table.sort(out, function(a, b)
      if a.data.sort and b.data.sort and a.data.sort ~= b.data.sort then return a.data.sort < b.data.sort end
      return a.item_id < b.item_id
    end)
    return out
  end
  function M.character_response(data, err, mgr)
    if not M.guard_ready or err or type(data) ~= 'table' or
      (data.error and data.error.code and data.error.code ~= 0) then return data end
    M.account(mgr)
    local out, seen = copy(data), {}
    out.characters = {}
    out.skins = merge_ids(data.skins, M.catalog.skin, 'skin')
    local sort_seen = {}
    out.character_sort = array(M.character_sort or data.character_sort)
    out.other_character_sort = array(M.other_character_sort or data.other_character_sort)
    out.hidden_characters = array(M.hidden_characters or data.hidden_characters)
    for _, id in ipairs(out.character_sort) do sort_seen[id] = true end
    for _, id in ipairs(out.other_character_sort) do sort_seen[id] = true end
    M.real_characters = {}
    for _, char in ipairs(data.characters or {}) do
      local cloned = copy(char)
      seen[char.charid], M.real_characters[char.charid] = true, true
      cloned.skin = M.skins[char.charid] or char.skin
      out.characters[#out.characters + 1] = cloned
    end
    for _, id in ipairs(M.catalog.character or {}) do
      local c = config('character', id)
      if c and not seen[id] and c.init_skin and config('skin', c.init_skin) then
        seen[id] = true
        out.characters[#out.characters + 1] = {
          charid = id, skin = M.skins[id] or c.init_skin, level = 5, exp = 0,
          is_upgraded = true, extra_emoji = {}, rewarded_level = {}, views = {},
        }
        if not sort_seen[id] then
          out.other_character_sort[#out.other_character_sort + 1] = id; sort_seen[id] = true
        end
      end
    end
    for _, char in ipairs(out.characters) do
      local c = config('character', char.charid)
      if M.character_sort and not sort_seen[char.charid] then
        out.other_character_sort[#out.other_character_sort + 1] = char.charid
        sort_seen[char.charid] = true
      end
      if c and c.init_skin then out.skins = merge_ids(out.skins, { c.init_skin }, 'skin') end
    end
    if M.choices.character_id and seen[M.choices.character_id] then out.main_character_id = M.choices.character_id end
    M.last_info = out
    M.sync_main(mgr, out)
    M.log('lobby character catalogue applied')
    return out
  end
  function M.apply(mgr)
    if not M.guard_ready or not mgr or type(mgr.characterInfo) ~= 'table' then return end
    M.account(mgr)
    if mgr.characterInfo ~= M.last_info then
      mgr.characterInfo = M.character_response(mgr.characterInfo, nil, mgr)
    end
    mgr.skin_map = mgr.skin_map or {}
    for _, id in ipairs(mgr.characterInfo.skins or {}) do mgr.skin_map[id] = 1 end
    M.sync_main(mgr, mgr.characterInfo)
  end
  function M.fetch_info(data)
    if not M.guard_ready or type(data) ~= 'table' then return data end
    local out = copy(data)
    if out.title_list then
      local titles = copy(out.title_list)
      titles.title_list = merge_ids(out.title_list.title_list, M.catalog.title, 'title')
      out.title_list = titles
    end
    return out
  end
  function M.sync_main(mgr, info)
    if not mgr or not info then return end
    local main = M.choices.character_id or info.main_character_id
    if M.choices.character_id then info.main_character_id = main end
    local skin = M.skins[main]
    for _, char in ipairs(info.characters or {}) do
      if char.charid == main then skin = skin or char.skin end
    end
    if skin and type(mgr.account_data) == 'table' then
      mgr.account_data = copy(mgr.account_data); mgr.account_data.avatar_id = skin
    end
  end
  -- Server notifications also call SetMainCharacterAndSkin. They must not record
  -- a new local choice or overwrite one made by an explicit cosmetic request.
  function M.main_args(charid, skin)
    local mgr = GameMgr and GameMgr.Inst
    if not M.guard_ready or not mgr then return charid, skin end
    M.account(mgr)
    local main = M.choices.character_id or (charid and charid > 0 and charid) or
      (mgr.characterInfo and mgr.characterInfo.main_character_id)
    if type(mgr.account_data) == 'table' then mgr.account_data = copy(mgr.account_data) end
    if M.choices.character_id then
      local c = config('character', main)
      charid, skin = main, M.skins[main] or (c and c.init_skin) or skin
    elseif M.skins[main] then skin = M.skins[main] end
    return charid, skin
  end
  function M.choose(charid, skin, main)
    local mgr = GameMgr and GameMgr.Inst
    M.account(mgr)
    if not config('character', charid) then return end
    if main then M.choices.character_id = charid end
    if valid_skin(charid, skin) then
      M.skins[charid] = skin
      for _, char in ipairs((mgr and mgr.characterInfo or {}).characters or {}) do
        if char.charid == charid then char.skin = skin end
      end
    end
    M.sync_main(mgr, mgr and mgr.characterInfo)
    M.save()
  end
  function M.refresh_room()
    local room = UI_FriendRoom and UI_FriendRoom.Inst
    if not room or not room.enable or not room.room_id or room.room_id <= 0 then return end
    room.players = M.room_players(room.players)
    for index, player in pairs(room.players or {}) do
      if player.account_id == M.account_id then
        -- The local request has no server NotifyRoomPlayerUpdate to refresh this.
        room:_refreshPlayerInfo(index)
        break
      end
    end
  end
  local function group_from_views(views)
    local group = {}
    for _, v in ipairs(views or {}) do
      group[v.slot] = { slot = v.slot, item_id = v.item_id or 0,
        type = v.type or 0, item_id_list = array(v.item_id_list) }
    end
    return group
  end
  local local_methods = {
    changeMainCharacter = true, changeCharacterSkin = true, changeCharacterView = true,
    updateCharacterSort = true, setHiddenCharacter = true, setRandomCharacter = true,
    changeCommonView = true, saveCommonViews = true, useCommonView = true,
    useTitle = true, setLoadingImage = true,
  }
  function M.request(module, method, data, callback)
    if module ~= 'Lobby' or not M.guard_ready then return false, callback end
    if method == 'fetchTitleList' and callback then
      return false, function(err, res, ...)
        local out = res
        if not err and type(res) == 'table' and not (res.error and res.error.code ~= 0) then
          out = copy(res); out.title_list = merge_ids(res.title_list, M.catalog.title, 'title')
        end
        return callback(err, out, ...)
      end
    end
    if not local_methods[method] then
      -- Progression is outside preview scope. A trial character must never be the
      -- subject of a gift/upgrade request, even if its native UI exposes that button.
      if (method == 'sendGiftToCharacter' or method == 'upgradeCharacter') and
        data and data.character_id and not (M.real_characters or {})[data.character_id] then
        if callback then callback('该角色仅供本地试用', { error = { code = 1 } }) end
        return true, callback
      end
      return false, callback
    end
    local mgr = GameMgr and GameMgr.Inst
    M.account(mgr)
    M.apply(mgr)
    if mgr and type(mgr.account_data) == 'table' then mgr.account_data = copy(mgr.account_data) end
    data = data or {}
    if method == 'changeMainCharacter' then
      local skin
      for _, char in ipairs((mgr and mgr.characterInfo or {}).characters or {}) do
        if char.charid == data.character_id then skin = char.skin end
      end
      M.choose(data.character_id, skin, true)
    elseif method == 'changeCharacterSkin' then
      M.choose(data.character_id, data.skin, false)
    elseif method == 'updateCharacterSort' then
      M.save_sort(data)
      if mgr and mgr.characterInfo then
        -- Let the native callback update its visible lists; keep login/reconnect
        -- source data consistent without touching the received server packet.
        local info = copy(mgr.characterInfo)
        info.character_sort = array(M.character_sort)
        info.other_character_sort = array(M.other_character_sort)
        info.hidden_characters = array(M.hidden_characters)
        mgr.characterInfo, M.last_info = info, info
      end
    elseif method == 'changeCharacterView' and data.character_id and data.slot ~= nil then
      M.character_views = M.character_views or {}
      local views = M.character_views[data.character_id] or {}
      views[data.slot] = { slot = data.slot, item_id = data.item_id }
      M.character_views[data.character_id] = views
    elseif method == 'changeCommonView' and mgr and data.slot ~= nil then
      local index = M.active_group or mgr.now_commonviews_index or 0
      local group = copy(M.groups[index] or (mgr.commonviews or {})[index + 1] or {})
      group[data.slot] = { slot = data.slot, item_id = data.value or 0, type = 0, item_id_list = {} }
      M.groups[index], M.active_group = group, index
      mgr.commonviews = mgr.commonviews or {}; mgr.commonviews[index + 1], mgr.commonview_slot = group, group
    elseif method == 'saveCommonViews' then
      M.groups[data.save_index or 0] = group_from_views(data.views)
      if data.is_use == 1 then M.active_group = data.save_index or 0 end
    elseif method == 'useCommonView' then
      M.active_group = data.index or 0
    elseif method == 'setLoadingImage' then
      M.loading_images = array(data.images)
      if mgr then mgr.loading_image = array(data.images) end
    elseif method == 'useTitle' then
      M.title = data.title
      if mgr and mgr.account_data then mgr.account_data.title = data.title end
    end
    if mgr and (method == 'useCommonView' or method == 'saveCommonViews') and M.active_group ~= nil then
      mgr.now_commonviews_index = M.active_group
      mgr.commonviews = mgr.commonviews or {}
      local group = M.groups[M.active_group] or mgr.commonviews[M.active_group + 1] or {}
      mgr.commonviews[M.active_group + 1], mgr.commonview_slot = group, group
    end
    local response = copy(data); response.error = { code = 0 }
    if callback then callback(nil, response) end
    if method == 'changeMainCharacter' or method == 'changeCharacterSkin' then M.refresh_room() end
    M.log('cosmetic requests kept local')
    return true, callback
  end
  function M.players(players, account, mode)
    local mgr = GameMgr and GameMgr.Inst
    if not M.guard_ready or not mgr or not GameUtility or not GameUtility.EMJ_Mode or
      mode ~= GameUtility.EMJ_Mode.play or not account or account == 0 or account ~= mgr.account_id then return players end
    M.account(mgr)
    local main = M.choices.character_id
    local out = array(players)
    for index, player in pairs(players or {}) do
      if player.account_id == account then
        local own = copy(player)
        local char = copy(player.character or {})
        if main and config('character', main) then
          char.charid = main
          local current
          for _, selected in ipairs((mgr.characterInfo or {}).characters or {}) do
            if selected.charid == main then current = selected.skin; break end
          end
          char.skin = M.skins[main] or current or config('character', main).init_skin
          char.level, char.is_upgraded = 5, true
        elseif M.skins[char.charid] then char.skin = M.skins[char.charid] end
        own.character, own.avatar_id = char, char.skin or player.avatar_id
        if M.character_views and M.character_views[char.charid] then
          char.views = {}
          for _, value in pairs(M.character_views[char.charid]) do char.views[#char.views + 1] = copy(value) end
        end
        if M.title ~= nil then own.title = M.title end
        local group = M.active_group ~= nil and (M.groups[M.active_group] or (mgr.commonviews or {})[M.active_group + 1])
        if group then
          own.views = {}
          for slot, view in pairs(group) do
            local id = view.item_id or 0
            if view.type == 1 and #(view.item_id_list or {}) > 0 then
              M.match_views = M.match_views or {}
              if M.match_views[slot] == nil then
                M.match_views[slot] = view.item_id_list[math.random(#view.item_id_list)]
              end
              id = M.match_views[slot]
            end
            own.views[#own.views + 1] = { slot = slot, item_id = id }
          end
        end
        out[index] = own
        M.log('local appearance character=' .. tostring(char.charid) .. ' skin=' .. tostring(char.skin))
        break
      end
    end
    return out
  end
  function M.room_players(players)
    local mgr = GameMgr and GameMgr.Inst
    if not players or not mgr or not M.guard_ready then return players end
    -- Room seats may contain holes. Preserve their indexes and every other seat.
    local out = copy(players)
    local trial = M.players(players, mgr.account_id, GameUtility.EMJ_Mode.play)
    for index, player in pairs(players) do
      if player.account_id == mgr.account_id then out[index] = trial[index] end
    end
    return out
  end
  function M.desktop(mgr)
    if not mgr or not mgr.active then return end
    local original = mgr.player_datas
    local players = M.players(original, mgr.myaccountid, mgr.mode)
    if players == original then return end
    mgr.player_datas = players
    for _, player in ipairs(players) do
      if player.account_id == mgr.myaccountid then mgr.myPlayerData = player; break end
    end
  end
  function M.preload_players(players)
    local mgr = GameMgr and GameMgr.Inst
    if not mgr or not M.guard_ready then return players end
    M.account(mgr)
    M.match_views = {}
    if not M.choices.character_id and next(M.skins) == nil and M.active_group == nil then return players end
    local trial = M.players(players, mgr.account_id, GameUtility.EMJ_Mode.play)
    if trial == players then return players end
    -- OpenMJRoom is also used for replay/spectator scenes. Load the union of real
    -- and trial resources here; only DesktopMgr's live-play path replaces rendering.
    local out = array(players)
    for index, value in ipairs(trial) do
      if value ~= players[index] then out[#out + 1] = value end
    end
    return out
  end
  if _MJLocalPreviewTrace then _MJLocalPreviewTrace('runtime initialized') end
end
