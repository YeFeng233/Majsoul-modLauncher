-- F10 stages every texture before replacing the current collection.
-- Native Unicode file IO supplies pixels; Lua never opens Chinese mod paths.
local U=UnityEngine
local function destroy(obj) if obj then pcall(U.GameObject.Destroy,obj) end end
local mods=@MODS@
local loaded,err=pcall(function()
 local function loadTexture(entry)
  local data=_MJCustomPixels(entry.index)
  assert(type(data)=='string','Missing image data')
  local function int(at) local a,b,c,d=data:byte(at,at+3);assert(d,'Truncated texture header');return a+b*256+c*65536+d*16777216 end
  local w,h=int(1),int(5)
  assert(w>0 and h>0 and #data==8+w*h*4,'Invalid texture data')
  entry.width=w;entry.height=h
  entry.texture=U.Texture2D(w,h,U.TextureFormat.RGBA32,false)
  entry.texture:LoadRawTextureData(data:sub(9));entry.texture:Apply(false,false)
 end
 for _,mod in ipairs(mods) do
  loadTexture(mod)
  for _,page in ipairs(mod.spine or {}) do loadTexture(page) end
   for _,kind in ipairs({'smallhead','bighead'}) do
    local entry=mod[kind]
    if entry then
     loadTexture(entry)
     entry.sprite=U.Sprite.Create(entry.texture,U.Rect(0,0,entry.width,entry.height),U.Vector2(0.5,0.5))
     assert(entry.sprite,'Failed to create avatar sprite')
    end
   end
 end
end)
local function releaseTextures()
 for _,mod in ipairs(mods) do
  destroy(mod.texture);mod.texture=nil
  for _,page in ipairs(mod.spine or {}) do destroy(page.texture);page.texture=nil end
   for _,kind in ipairs({'smallhead','bighead'}) do
    local entry=mod[kind]
    if entry then destroy(entry.sprite);entry.sprite=nil;destroy(entry.texture);entry.texture=nil end
   end
 end
end
if not loaded then releaseTextures();error(err)end
if MJCustomPortrait then MJCustomPortrait.close() end
local S={enabled=true,mods=mods,records={},heads=setmetatable({},{__mode='k'})}
MJCustomHeadTargets=MJCustomHeadTargets or setmetatable({},{__mode='k'})
local headTargets=MJCustomHeadTargets
MJCustomPortrait=S
local function restore(record)
 if record.dynamic then
  for graphic,state in pairs(record.graphics) do pcall(function()
   if state.dictionary then
    local overrides=graphic.CustomTextureOverride
    if overrides:ContainsKey(state.source) and overrides:get_Item(state.source)==state.replacement then
     if state.hadOriginal then overrides:set_Item(state.source,state.original) else overrides:Remove(state.source) end
     graphic:SetMaterialDirty()
    end
   end
   if graphic.OverrideTexture==state.replacement then
    if state.dictionary then graphic.OverrideTexture=state.overrideOriginal else graphic.OverrideTexture=state.original end
    graphic:SetMaterialDirty()
   end
  end) end
  return
 end
 local same=false
 pcall(function()same=record.version==record.owner.targetVersion end)
 if same then for obj,active in pairs(record.saved) do pcall(function()obj:SetActive(active)end) end end
 destroy(record.go)
end
S.restore=restore
function S.close()
 if not S.enabled then return end
 S.enabled=false
 for _,record in pairs(S.records) do restore(record) end
 S.records={}
 if S.restoreHeads then S.restoreHeads() end
 releaseTextures()
end
local function alive(skin,preparing)
 return skin and skin.image and skin.image.transform.parent.gameObject.activeInHierarchy and skin.active~=false and (not skin.loadingSkin or skin==preparing)
end
local function choose(skin)
 local p=skin.targetSkinParams or {}
 local id=tonumber(p.skinId or skin.skin_id or skin.skinId)
 if not id or id==0 then return nil end
 local config=ExcelMgr.GetData('item_definition','skin',id)
 if not config then return nil end
 local fallback
 for _,mod in ipairs(S.mods)do
  if mod.character==tonumber(config.character_id) then
   if mod.skin==id then return mod end
   if mod.skin==0 then fallback=mod end
  end
 end
 return fallback
end
function S.restoreHead(image)
 local record=S.heads[image]
 if record then
  pcall(function()if image.sprite==record.replacement then image.sprite=record.original end end)
  S.heads[image]=nil
 end
end
function S.restoreHeads()
 for image in pairs(S.heads) do S.restoreHead(image) end
end
function S.trackHead(image,id,kind)
 S.restoreHead(image)
 if kind=='smallhead' or kind=='bighead' then headTargets[image]={skin=tonumber(id),kind=kind}
 else headTargets[image]=nil end
end
function S.requestHead(image,path)
 if type(path)~='string' then return end
 local kind=path:match('/(smallhead)/smallhead$') or path:match('/(bighead)/bighead$')
 local target=headTargets[image]
 if not kind then S.restoreHead(image);headTargets[image]=nil;return end
 local directory=path:sub(1,#path-#kind*2-2)
 if target and target.kind==kind and SkinTool and SkinTool.GetSkinIconDir and SkinTool.GetSkinIconDir(target.skin)==directory then
  target.path=path;return
 end
 local id=tonumber(directory:match('^deco/character/(%d+)$'))
 if not id and SkinTool and SkinTool.GetSkinIconDir then
  for _,mod in ipairs(S.mods) do
   if mod.skin>0 and SkinTool.GetSkinIconDir(mod.skin)==directory then id=mod.skin;break end
  end
 end
 if not id and SkinTool then
  for key,value in pairs(SkinTool.skinIconDirCache or {}) do
   if value==directory then id=tonumber(key);if id then break end end
  end
 end
 S.trackHead(image,id,kind)
 headTargets[image].path=path
end
function S.applyHead(image,target)
 if not S.enabled or not target or headTargets[image]~=target then return end
 local mod=choose({targetSkinParams={skinId=target.skin}})
 local entry=mod and mod[target.kind]
 if not entry then S.restoreHead(image);return end
 local record=S.heads[image]
 if not record then
  record={original=image.sprite,replacement=entry.sprite};S.heads[image]=record
 elseif image.sprite~=record.replacement then record.original=image.sprite end
 record.replacement=entry.sprite
 image.sprite=entry.sprite
end
local function headCall(method,...)
 local current=MJCustomPortrait
 if current and current[method] then
  local ok,err=pcall(current[method],...)
  if not ok then current.last_error=tostring(err) end
 end
end
local function installHeadHooks()
 if Tools and Tools.SetHeadImg and not Tools._mjcustom_head_hook then
  Tools._mjcustom_head_hook=true
  local original=Tools.SetHeadImg
  Tools.SetHeadImg=function(image,id,kind,...)
   headCall('trackHead',image,id,kind)
   local function done(...)
    headCall('applyHead',image,headTargets[image])
    return ...
   end
   return done(original(image,id,kind,...))
  end
 end
 if ImageMgr and ImageMgr.SetTextureAsync and not ImageMgr._mjcustom_head_hook then
  ImageMgr._mjcustom_head_hook=true
  local original=ImageMgr.SetTextureAsync
  ImageMgr.SetTextureAsync=function(image,path,tag,complete,...)
   headCall('requestHead',image,path)
   local target=headTargets[image]
   if not target then return original(image,path,tag,complete,...) end
   local function done(...)
    headCall('applyHead',image,target)
    if complete then return complete(...) end
   end
   return original(image,path,tag,done,...)
  end
 end
end
function S.updateHeads()
 installHeadHooks()
 -- Already visible cards can predate these hooks. The client caches their
 -- paths and skips SetHeadImg on refresh, so register them without a reload.
 local selection=UI_LiaosheSelect and UI_LiaosheSelect.Inst
 for row,path in pairs(selection and selection.map_go_headpath or {}) do
  pcall(function()
   if row.activeInHierarchy then
    local image=row.transform:Find('btn'):Find('head'):GetComponent(typeof(UnityEngine.UI.Image))
    local target=image and headTargets[image]
    if image and (not target or target.path~=path) then S.requestHead(image,path) end
   end
  end)
 end
 for image,target in pairs(headTargets) do
  local ok=pcall(S.applyHead,image,target)
  if not ok then S.heads[image]=nil;headTargets[image]=nil end
 end
end
local function candidates(preparing)
 local out={}
 local mgr=GameMgr and GameMgr.Inst
 local account=mgr and mgr.account_id
 if not account or account<=0 then return out end
 local function add(skin,kind)
  local ok,eligible=pcall(function()return skin and alive(skin,preparing) and choose(skin)end)
  if ok and eligible then out[skin]={kind=kind,mod=eligible} end
 end
 local lobby=UI_Lobby and UI_Lobby.Inst
 if lobby and lobby.container_illust then add(lobby.container_illust.character_skin,'lobby') end
 local dorm=UI_LiaosheMain and UI_LiaosheMain.Inst
 if dorm and dorm.illust then add(dorm.illust.illust,'dorm') end
 local wardrobe=UI_LiaoSheChangeSkin and UI_LiaoSheChangeSkin.Inst
 if wardrobe then for _,item in pairs(wardrobe.skinItemList or {}) do add(item.illustration,'dorm_preview') end end
 local room=UI_FriendRoom and UI_FriendRoom.Inst
 if room then for index,player in pairs(room.players or {}) do
  if player.account_id==account then
   local cell=(room.playerCells or {})[index]
   if cell then add(cell.illust,'room');add(cell.illust_spine,'room') end
  end
 end end
 -- Player profiles are non-singleton UIBase windows: there is no .Inst.
 -- Include every live self profile, including nested windows from comments.
 local uiMgr=UIMgr and UIMgr.Inst
 local profiles=uiMgr and uiMgr.uiInstanceDict and uiMgr.uiInstanceDict[(UI_PlayerInfo and UI_PlayerInfo.className) or 'UI_PlayerInfo']
 for _,profile in ipairs(profiles or {}) do
  if profile.accountId==account then
   add(profile.illustration,'profile');add(profile.illustrationSpine,'profile')
  end
 end
 local desktop=DesktopMgr and DesktopMgr.Inst
 local play=desktop and GameUtility and GameUtility.EMJ_Mode and desktop.mode==GameUtility.EMJ_Mode.play
 if play then
  local ending=UI_GameEnd and UI_GameEnd.Inst
  if ending then add(ending.character,'ending') end
  local ui=UI_DesktopInfo and UI_DesktopInfo.Inst
  if ui then for position,info in pairs(ui.player_infos or {}) do
   local seat=desktop:localPosition2Seat(position)
   local player=(desktop.player_datas or {})[seat]
   if player and player.account_id==account then add(info.illust,'match') end
  end end
  -- Includes the winner's full illustration and final-results components.
  for _,skin in ipairs(UI_Character_Skin.instances or {}) do
   local params=skin.targetSkinParams or {}
   if params.accountId==account and not out[skin] then add(skin,'match') end
  end
 end
 return out
end
-- Use the loaded illustration geometry, not the build-time placeholder rect.
-- Keeping the original parent preserves the game's mask and carousel zoom.
local function layout(record)
 local skin,mod,kind=record.owner,record.mod,record.kind
 local image=skin.image.rectTransform
 local rect=record.raw.rectTransform
 if rect.parent~=image.parent then rect:SetParent(image.parent,false) end
 rect.anchorMin=image.anchorMin;rect.anchorMax=image.anchorMax;rect.pivot=image.pivot
 rect.localScale=image.localScale;rect.localRotation=image.localRotation
 local w,h=image.rect.width,image.rect.height
 local x,y=image.anchoredPosition.x,image.anchoredPosition.y
 -- An animated skin may leave the static image at its previous geometry.
 if skin.targetShowSpine and skin.originRect and Tools then
  local ok,data=pcall(function()
   local params=skin.targetSkinParams or {}
   local id=params.skinId or skin.skin_id
   local config=skin.targetSkinInfo
   local fresh=SkinTool and SkinTool.IsNewTypeSkin and config and SkinTool.IsNewTypeSkin(config)
   local data=fresh and Tools.GetIllustDataByType(id,skin.type,skin.originRect,skin.index,skin.force_offset)
   return data or Tools.GetSkinData(id,skin.originRect,params.partName or 'full',nil,skin.location)
  end)
  if ok and data and data.width and data.height and data.width>0 and data.height>0 then
   w,h=data.width,data.height
   x=skin.originRect.anchoredPosition.x+(data.delta_x or 0)
   y=skin.originRect.anchoredPosition.y-(data.delta_y or 0)
  end
 end
 local options=mod.options or {}
 h=options[kind..'_height'] or h
 w=h*mod.width/mod.height
 x=x+(options[kind..'_x'] or 0);y=y+(options[kind..'_y'] or 0)
 -- sizeDelta is relative to the anchors; account for stretched transforms too.
 local parent=image.parent
 local ax=(image.anchorMax.x or 0)-(image.anchorMin.x or 0)
 local ay=(image.anchorMax.y or 0)-(image.anchorMin.y or 0)
 rect.sizeDelta=U.Vector2(w-parent.rect.width*ax,h-parent.rect.height*ay)
 rect.anchoredPosition=U.Vector2(x,y)
 pcall(function()record.raw.color=skin.image.color end)
end
local function create(skin,kind,mod)
 local go=U.GameObject('MJCustomPortrait')
 go:SetActive(false)
 local record={go=go,owner=skin,version=skin.targetVersion,saved={},kind=kind,mod=mod}
 local raw=go:AddComponent(typeof(U.UI.RawImage));record.raw=raw
 raw.raycastTarget=false;raw.texture=mod.texture
 layout(record)
 raw.rectTransform:SetAsLastSibling();go:SetActive(true)
 return record
end
local function hide(record)
 local skin=record.owner
 local function one(obj)
  if not obj or obj==record.go then return end
  if record.saved[obj]==nil or obj.activeSelf then record.saved[obj]=obj.activeSelf end
  obj:SetActive(false)
 end
 one(skin.image.gameObject);one(skin.spine_root)
 if skin.spine_raw_image then one(skin.spine_raw_image.gameObject) end
 record.go:SetActive(true)
end
-- Source extraction layers need not equal the runtime SkeletonGraphic order.
-- Match the atlas identity first; use the supplied layer only to disambiguate.
local function spineGraphic(record,page)
 local matches={}
 for index,layer in ipairs(record.owner.spine_list or {}) do
  local graphic=layer.spine_graphic
  local state=record.graphics[graphic]
  local texture=state and state.source or (graphic and graphic.mainTexture)
  if texture and tostring(texture.name):lower()==page.name:lower() and texture.width==page.width and texture.height==page.height then
   if index==page.layer then return graphic,index end
   matches[#matches+1]={graphic=graphic,index=index}
  end
 end
 if #matches==1 then return matches[1].graphic,matches[1].index end
end
local function updateSpine(record)
 local wanted={}
 record.pages={}
 for _,page in ipairs(record.mod.spine or {}) do
  local graphic,index=spineGraphic(record,page)
  if graphic then
   local state=record.graphics[graphic]
   if not state then
    local texture=graphic.mainTexture
    state={source=texture,replacement=page.texture,dictionary=graphic.allowMultipleCanvasRenderers==true,overrideOriginal=graphic.OverrideTexture}
    if state.dictionary then
     local overrides=graphic.CustomTextureOverride
     state.hadOriginal=overrides:ContainsKey(texture)
     if state.hadOriginal then state.original=overrides:get_Item(texture) end
    else state.original=graphic.OverrideTexture end
    record.graphics[graphic]=state
   end
   if state.dictionary then
    local overrides=graphic.CustomTextureOverride
    if not overrides:ContainsKey(state.source) or overrides:get_Item(state.source)~=page.texture then
     overrides:set_Item(state.source,page.texture);graphic:SetMaterialDirty()
    end
   end
   if graphic.OverrideTexture~=page.texture then
    graphic.OverrideTexture=page.texture;graphic:SetMaterialDirty()
   end
   record.pages[page.name]={layer=index,applied=true,dictionary=state.dictionary}
   wanted[graphic]=true
  else record.pages[page.name]={applied=false} end
 end
 for graphic,state in pairs(record.graphics) do
  if not wanted[graphic] then
   restore({dynamic=true,graphics={[graphic]=state}});record.graphics[graphic]=nil
  end
 end
end
function S.update(preparing)
 if not S.enabled then return end
 S.updateHeads()
 local wanted=candidates(preparing)
 for skin,record in pairs(S.records) do
  local valid=false
  pcall(function()valid=wanted[skin]~=nil and record.mod==wanted[skin].mod and record.kind==wanted[skin].kind and record.dynamic==(skin.targetShowSpine==true) and record.version==skin.targetVersion end)
  if not valid then restore(record);S.records[skin]=nil end
 end
 for skin,target in pairs(wanted) do
  local record=S.records[skin]
  if not record then
   if skin.targetShowSpine then
    record={owner=skin,version=skin.targetVersion,kind=target.kind,mod=target.mod,dynamic=true,graphics={}}
   else
    record=create(skin,target.kind,target.mod);record.dynamic=false
   end
   S.records[skin]=record
  end
  if record.dynamic then updateSpine(record) else layout(record);hide(record) end
 end
end
-- These callbacks run synchronously before control returns to Unity rendering.
-- Resolve the current collection so F10 never leaves an old texture captured.
local function beforeRender(skin)
 local current=MJCustomPortrait
 if current and current.enabled then
  local ok,err=pcall(current.update,skin)
  if not ok then current.last_error=tostring(err) end
 end
end
if not UI_Character_Skin._mjcustom_first_frame then
 local original=UI_Character_Skin.SetSkin
 if original then
  UI_Character_Skin._mjcustom_first_frame=true
  UI_Character_Skin.SetSkin=function(self,params)
   local prepared={}
   for key,value in pairs(params) do prepared[key]=value end
   local complete=params.complete
   prepared.complete=function(...)
    if not select(1,...) then beforeRender(self) end
    if complete then return complete(...) end
   end
   return original(self,prepared)
  end
 end
end
-- Keep overlays synchronized with explicit hide/clear calls between polling ticks.
if UI_Character_Skin._mjcustom_visibility_version~=3 then
 UI_Character_Skin._mjcustom_visibility=true
 UI_Character_Skin._mjcustom_visibility_version=3
 for _,name in ipairs({'SetActive','Clear','ClearWithoutCImage'}) do
  local original=UI_Character_Skin[name]
  if original then UI_Character_Skin[name]=function(self,...)
   local s=MJCustomPortrait
   local record=s and s.records[self]
   if record then s.restore(record);s.records[self]=nil end
   if name=='SetActive' then
    original(self,...)
    beforeRender(self)
    return
   end
   return original(self,...)
  end end
 end
end
S.update()
return true
