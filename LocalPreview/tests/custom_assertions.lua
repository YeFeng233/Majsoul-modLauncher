-- Test ownership and scene lifecycle using the actual shipping portrait script.
local saved={UnityEngine=UnityEngine,GameMgr=GameMgr,ExcelMgr=ExcelMgr,GameUtility=GameUtility,DesktopMgr=DesktopMgr,
 Tools=Tools,SkinTool=SkinTool,UI_Lobby=UI_Lobby,UI_LiaosheMain=UI_LiaosheMain,UI_LiaoSheChangeSkin=UI_LiaoSheChangeSkin,UI_FriendRoom=UI_FriendRoom,UI_DesktopInfo=UI_DesktopInfo,UI_GameEnd=UI_GameEnd,UI_Character_Skin=UI_Character_Skin,MJCustomPortrait=MJCustomPortrait,ImageMgr=ImageMgr,MJCustomHeadTargets=MJCustomHeadTargets}
local oldpixels=_MJCustomPixels
local objects={}
local function object()
 local o={activeSelf=true,activeInHierarchy=true}
 function o:SetActive(v) self.activeSelf=v end
 return o
end
local function transform()
 local t={gameObject=object(),anchorMin={x=0.5,y=0.5},anchorMax={x=0.5,y=0.5},pivot={x=0,y=1},localScale={x=1,y=1,z=1},localRotation={x=0,y=0,z=0,w=1},rect={width=500,height=1000},anchoredPosition={x=0,y=0}}
 function t:SetParent(p) self.parent=p end
 function t:SetAsLastSibling()end
 return t
end
local function vec(x,y,z)return{x=x,y=y,z=z}end
local function go()
 local o=object();o.transform=transform()
 function o:AddComponent()return{rectTransform=self.transform}end
 objects[#objects+1]=o;return o
end
local gameObject=setmetatable({Destroy=function(o)o.destroyed=true end},{__call=function()return go()end})
UnityEngine={GameObject=gameObject,UI={RawImage={}},Vector2=vec,Vector3=vec,Rect=function(x,y,w,h)return{x=x,y=y,width=w,height=h}end,Sprite={Create=function(texture,rect,pivot)return{texture=texture,rect=rect,pivot=pivot}end},TextureFormat={RGBA32=4},Texture2D=function()
 return{LoadRawTextureData=function(self,data)self.pixels=data end,Apply=function()end}
end}
local oldtypeof=typeof;typeof=function(t)return t end
local pixels=string.char(1,0,0,0,1,0,0,0,255,0,0,255)
local blue=string.char(1,0,0,0,1,0,0,0,0,0,255,255)
_MJCustomPixels=function(index)if index==1 then return pixels else return blue end end
GameMgr={Inst={account_id=10}}
ExcelMgr={GetData=function(_,section,id)if section=='skin' then if id==401701 or id==401702 then return{character_id=200017}elseif id==401801 then return{character_id=200018}end end end}
GameUtility={EMJ_Mode={play=1,paipu=2}}
local function skin(account)
 local parent=transform();local image=transform();image.parent=parent
 parent.name='father';parent.parent=transform();parent.parent.name='mask'
 return{image={rectTransform=image,transform=image,gameObject=object()},illust_base=parent,
  spine_root=object(),active=true,targetVersion=1,targetSkinParams={skinId=401701,accountId=account}}
end
local lobby,own,other=skin(10),skin(10),skin(20)
local dorm,wardrobe,another=skin(10),skin(10),skin(10)
another.targetSkinParams.skinId=400101
UI_LiaosheMain={Inst={illust={illust=dorm}}}
UI_LiaoSheChangeSkin={Inst={skinItemList={{illustration=wardrobe},{illustration=another}}}}
UI_Character_Skin={instances={own,other},_mjcustom_visibility=true,
 SetSkin=function(s,params)
  s.targetSkinParams=params;s.targetVersion=s.targetVersion+1
  s.loadingSkin=true;s.targetShowSpine=params.dynamic or false
  s.pending=function(canceled)
   if canceled then return params.complete(true) end
   s.image.gameObject:SetActive(not s.targetShowSpine)
   s.spine_root:SetActive(s.targetShowSpine)
   if not s.targetShowSpine then s.loadingSkin=false end
   params.complete()
   s.loadingSkin=false
  end
  return 'scheduled',nil,'complete'
 end,
 SetActive=function(s,b)
  assert(not(MJCustomPortrait and MJCustomPortrait.records[s]),'legacy visibility hook received a new-format record')
  s.active=b
 end,Clear=function(s)s.targetSkinParams={}end}
UI_Lobby={Inst={container_illust={character_skin=lobby}}}
UI_FriendRoom={Inst={players={[1]={account_id=10},[2]={account_id=20}},playerCells={[1]={illust=own},[2]={illust=other}}}}
UI_DesktopInfo=nil;DesktopMgr=nil;MJCustomPortrait=nil;UI_GameEnd=nil
local function reload(source)assert(loadstring(source or MJCustomPortraitSource))()end
reload()
assert(MJCustomPortrait.records[lobby] and MJCustomPortrait.records[own],'lobby/own room portraits missing')
assert(not MJCustomPortrait.records[other] and other.image.gameObject.activeSelf,'opponent portrait was replaced')
assert(MJCustomPortrait.records[dorm] and MJCustomPortrait.records[wardrobe],'dorm or wardrobe preview was missed')
assert(not MJCustomPortrait.records[another],'unconfigured character in wardrobe was replaced')
-- Preserve the loaded artwork geometry, rather than fitting a whole image in
-- a placeholder/mask; the latter shrinks portraits and carousel cards.
local source=lobby.image.rectTransform
source.rect.height=1299.56;source.anchoredPosition={x=-155.94,y=775.09}
source.localScale={x=-0.98,y=0.98,z=1};source.localRotation={x=0,y=0,z=0.2,w=0.98}
MJCustomPortrait.update()
local aligned=MJCustomPortrait.records[lobby].raw.rectTransform
assert(aligned.parent==source.parent and aligned.localScale==source.localScale and aligned.localRotation==source.localRotation,'original transform/flip was not preserved')
assert(aligned.sizeDelta.y==1299.56 and aligned.sizeDelta.x==1299.56,'loaded illustration height or custom image aspect was lost')
assert(aligned.anchoredPosition.x==-155.94 and aligned.anchoredPosition.y==775.09 and aligned.pivot==source.pivot,'loaded illustration position/pivot was lost')
source.rect.height=1500;source.anchoredPosition.y=800;MJCustomPortrait.update()
assert(aligned.sizeDelta.y==1500 and aligned.anchoredPosition.y==800,'layout did not follow later game geometry changes')
source.anchorMin={x=0,y=0};source.anchorMax={x=1,y=1};MJCustomPortrait.update()
assert(aligned.sizeDelta.x==1000 and aligned.sizeDelta.y==500,'stretched anchors were counted twice')
source.anchorMin={x=0.5,y=0.5};source.anchorMax={x=0.5,y=0.5}
local previewRect=MJCustomPortrait.records[wardrobe].raw.rectTransform
assert(previewRect.parent==wardrobe.image.rectTransform.parent,'carousel lost its original zoom parent')
local rule=MJCustomPortrait.records[dorm].mod
rule.options.dorm_height=875;rule.options.dorm_x=12;rule.options.dorm_y=-5;MJCustomPortrait.update()
local adjusted=MJCustomPortrait.records[dorm].raw.rectTransform
assert(adjusted.sizeDelta.y==875 and adjusted.anchoredPosition.x==12 and adjusted.anchoredPosition.y==-5,'layout overrides were not applied relative to the original')
rule.options.dorm_height=nil;rule.options.dorm_x=nil;rule.options.dorm_y=nil
-- Animated originals resolve their own skin geometry, not a stale static image.
local beforeTools,beforeSkinTool=Tools,SkinTool
Tools={GetSkinData=function()return{width=600,height=1500,delta_x=-10,delta_y=25}end};SkinTool=nil
dorm.targetShowSpine=true;dorm.originRect={anchoredPosition={x=100,y=400}}
MJCustomPortrait.update()
assert(MJCustomPortrait.records[dorm].dynamic and not MJCustomPortrait.records[dorm].go,'animated skin was flattened into an overlay')
assert(dorm.image.gameObject.activeSelf and dorm.spine_root.activeSelf,'dynamic fallback hid original components')
-- A matching atlas overrides the instance without changing animation or shared assets.
local original={name='test_atlas',width=1,height=1}
local animation={time=2.5};local shared={mainTexture=original}
local graphic={mainTexture=original,AnimationState=animation,material=shared,SetMaterialDirty=function(self)self.dirty=true end}
local dynamic=MJCustomPortrait.records[dorm]
local page={layer=2,name='test_atlas',width=1,height=1,texture={custom=true}}
dynamic.mod.spine={page};dorm.spine_list={{},{spine_graphic=graphic}}
MJCustomPortrait.update()
assert(graphic.OverrideTexture==page.texture and graphic.dirty,'matching Spine atlas was not applied')
assert(graphic.AnimationState==animation and animation.time==2.5 and shared.mainTexture==original,'animation or shared material was modified')
page.width=2;MJCustomPortrait.update()
assert(graphic.OverrideTexture==nil,'mismatched atlas dimensions were applied or previous override was retained')
page.width=1;MJCustomPortrait.update();assert(graphic.OverrideTexture==page.texture)
local oldGraphic=graphic
local second={mainTexture=original,SetMaterialDirty=graphic.SetMaterialDirty}
dorm.spine_list[2].spine_graphic=second;MJCustomPortrait.update()
assert(oldGraphic.OverrideTexture==nil and second.OverrideTexture==page.texture,'reloaded Spine component leaked a texture override')
dorm.targetShowSpine=false;dorm.originRect=nil;Tools=beforeTools;SkinTool=beforeSkinTool
MJCustomPortrait.update()
assert(second.OverrideTexture==nil and not MJCustomPortrait.records[dorm].dynamic,'switching to static retained the dynamic override')
dorm.targetShowSpine=true;MJCustomPortrait.update();assert(second.OverrideTexture==page.texture)
UI_Character_Skin.SetActive(dorm,false)
assert(second.OverrideTexture==nil and not MJCustomPortrait.records[dorm],'visibility hook retained a stale restore handler')
UI_Character_Skin.SetActive(dorm,true);MJCustomPortrait.update();assert(second.OverrideTexture==page.texture)
local dictionary={values={}}
function dictionary:ContainsKey(key)return self.values[key]~=nil end
function dictionary:get_Item(key)return self.values[key]end
function dictionary:set_Item(key,value)self.values[key]=value end
function dictionary:Remove(key)self.values[key]=nil end
local previousTexture={previous=true};dictionary:set_Item(original,previousTexture)
local multi={mainTexture=original,allowMultipleCanvasRenderers=true,CustomTextureOverride=dictionary,SetMaterialDirty=graphic.SetMaterialDirty}
dorm.spine_list={{spine_graphic=multi}};MJCustomPortrait.update()
assert(second.OverrideTexture==nil,'reordering the layers retained an old override')
assert(dictionary:get_Item(original)==page.texture and multi.OverrideTexture==page.texture,'multi-renderer did not cover both dictionary and single-submesh paths')
assert(MJCustomPortrait.records[dorm].pages.test_atlas.layer==1,'atlas name did not resolve a changed runtime layer index')
page.name='wrong_atlas';MJCustomPortrait.update()
assert(dictionary:get_Item(original)==previousTexture and multi.OverrideTexture==nil,'mismatch did not restore both texture override paths')
page.name='test_atlas';MJCustomPortrait.update();assert(dictionary:get_Item(original)==page.texture)
MJCustomPortrait.close();assert(dictionary:get_Item(original)==previousTexture and multi.OverrideTexture==nil and page.texture==nil,'F11 failed to restore both bindings or release atlas texture')
reload();dorm.targetShowSpine=false;MJCustomPortrait.update()

local exact=MJCustomPortrait.records[wardrobe]
assert(exact.mod.skin==401701 and exact.raw.texture.pixels==blue:sub(9),'exact skin did not override character fallback')
wardrobe.targetSkinParams.skinId=401702;MJCustomPortrait.update()
local fallback=MJCustomPortrait.records[wardrobe]
assert(fallback.mod.skin==0 and fallback.raw.texture.pixels==pixels:sub(9),'character fallback did not use its separate image')
assert(exact.go.destroyed,'changing the selected skin leaked the previous overlay')
another.targetSkinParams.skinId=401801;MJCustomPortrait.update()
assert(MJCustomPortrait.records[another].mod.character==200018,'different configured character was not rendered simultaneously')
wardrobe.targetSkinParams.skinId=401701
another.targetSkinParams.skinId=400101;MJCustomPortrait.update()
dorm.targetSkinParams.skinId=400101;MJCustomPortrait.update()
assert(not MJCustomPortrait.records[dorm] and dorm.image.gameObject.activeSelf,'switching dorm character did not remove overlay')
dorm.targetSkinParams.skinId=401701;MJCustomPortrait.update()
assert(MJCustomPortrait.records[dorm],'switching back in dorm did not restore overlay')
MJCustomPortrait.close()
assert(lobby.image.gameObject.activeSelf and own.image.gameObject.activeSelf,'F11 did not restore original components')
assert(dorm.image.gameObject.activeSelf and wardrobe.image.gameObject.activeSelf,'F11 did not restore dorm previews')
MJCustomPortrait.update();assert(next(MJCustomPortrait.records)==nil,'F11 unexpectedly re-enabled mods')
for _,mod in ipairs(MJCustomPortrait.mods)do assert(mod.texture==nil,'F11 retained a custom texture')end
UI_Lobby=nil;UI_FriendRoom=nil;UI_LiaosheMain=nil;UI_LiaoSheChangeSkin=nil
local match,opponent=skin(10),skin(20)
DesktopMgr={Inst={mode=1,player_datas={[1]={account_id=10},[2]={account_id=20}},localPosition2Seat=function(_,p)return p end}}
UI_DesktopInfo={Inst={player_infos={{illust=match},{illust=opponent}}}}
UI_Character_Skin.instances={match,opponent}
UI_Character_Skin.instances[3]={targetSkinParams={skinId=401701,accountId=10},image=setmetatable({},{__index=function()error('destroyed Unity object')end})}
local ending=skin(nil);UI_GameEnd={Inst={character=ending}}
reload();assert(MJCustomPortrait.records[match] and not MJCustomPortrait.records[opponent],'match ownership filtering failed')
assert(MJCustomPortrait.records[ending],'own final-result portrait with no account parameter was missed')
assert(MJCustomPortrait.records[match].raw.rectTransform.parent==match.image.rectTransform.parent,'cut-in changed the original mask/scale hierarchy')
local previous=MJCustomPortrait
pixels='invalid';local before=#objects;assert(not pcall(reload));assert(MJCustomPortrait==previous and previous.enabled,'bad reload discarded current portrait')
pixels=string.char(1,0,0,0,1,0,0,0,255,0,0,255)
match.active=false;MJCustomPortrait.update();assert(not MJCustomPortrait.records[match],'hidden scene left an overlay')
match.active=true;MJCustomPortrait.update();assert(MJCustomPortrait.records[match],'returning to a scene did not restore overlay')
DesktopMgr.Inst.mode=2;MJCustomPortrait.update();assert(next(MJCustomPortrait.records)==nil,'replay was modified')
MJCustomPortrait.close()
-- A successful F10 with no enabled mods must restore every original.
DesktopMgr.Inst.mode=1;reload();local old=MJCustomPortrait
local empty=MJCustomPortraitSource:gsub('local mods=.-\nlocal loaded','local mods={}\nlocal loaded',1)
reload(empty);assert(not old.enabled and next(MJCustomPortrait.records)==nil,'removing/turning off all mods retained overlays')
assert(match.image.gameObject.activeSelf and ending.image.gameObject.activeSelf,'empty reload failed to restore originals')
-- Narrowing a character-wide rule must restore the other skins, including
-- pooled wardrobe components whose targetVersion did not change.
local exactLobby,exactCard,contract,otherRole=skin(10),skin(10),skin(10),skin(10)
contract.targetSkinParams.skinId=401702;otherRole.targetSkinParams.skinId=401801
UI_Lobby={Inst={container_illust={character_skin=exactLobby}}}
UI_LiaosheMain={Inst={illust={illust=contract}}}
UI_LiaoSheChangeSkin={Inst={skinItemList={{illustration=exactCard},{illustration=contract},{illustration=otherRole}}}}
reload();assert(MJCustomPortrait.records[contract] and MJCustomPortrait.records[otherRole],'broad fixture missing')
local broad=MJCustomPortrait
local narrow=MJCustomPortraitSource:gsub('local mods=.-\nlocal loaded','local mods={{character=200017,skin=401701,index=1,options={}}}\nlocal loaded',1)
reload(narrow)
assert(not broad.enabled and MJCustomPortrait.records[exactLobby] and MJCustomPortrait.records[exactCard],'exact skin was not preserved')
assert(not MJCustomPortrait.records[contract] and contract.image.gameObject.activeSelf and contract.spine_root.activeSelf,'contract skin was replaced by a default-skin-only rule')
assert(not MJCustomPortrait.records[otherRole] and otherRole.image.gameObject.activeSelf,'unconfigured role was not restored')
exactCard.targetSkinParams.skinId=401702;MJCustomPortrait.update()
assert(not MJCustomPortrait.records[exactCard] and exactCard.image.gameObject.activeSelf,'pooled preview kept an unrelated skin overlay')
exactCard.targetSkinParams.skinId=401701;MJCustomPortrait.update()
assert(MJCustomPortrait.records[exactCard],'returning to the configured skin did not restore its mod')
-- Completion must bind textures before the caller sees the loaded view, even
-- though the dynamic client callback clears loadingSkin only after completion.
local callbackCount=0
local complete=function(...)
 assert(select('#',...)==0,'completion argument count changed')
 callbackCount=callbackCount+1
 local record=MJCustomPortrait.records[exactLobby]
 assert(record and not exactLobby.image.gameObject.activeSelf,'static original was visible at completion')
end
local params={skinId=401701,accountId=10,complete=complete}
local a,b,c=UI_Character_Skin.SetSkin(exactLobby,params)
assert(a=='scheduled' and b==nil and c=='complete','SetSkin return values changed')
assert(params.complete==complete and exactLobby.targetSkinParams~=params,'caller parameters were mutated')
exactLobby.pending();assert(callbackCount==1,'original completion was lost')
local firstFramePage={layer=2,name='first_frame',width=1,height=1,texture={}}
MJCustomPortrait.mods[1].spine={firstFramePage}
local firstFrameGraphic={mainTexture={name='first_frame',width=1,height=1}}
exactLobby.spine_list={{spine_graphic=firstFrameGraphic}}
local dynamicParams={skinId=401701,accountId=10,dynamic=true,complete=function(...)
 assert(select('#',...)==0 and exactLobby.loadingSkin,'dynamic completion timing changed')
 assert(firstFrameGraphic.OverrideTexture==firstFramePage.texture,'dynamic original reached completion before replacement')
 callbackCount=callbackCount+1
end}
UI_Character_Skin.SetSkin(exactLobby,dynamicParams);exactLobby.pending()
assert(callbackCount==2,'dynamic completion was lost')
UI_Character_Skin.SetActive(exactLobby,false)
assert(firstFrameGraphic.OverrideTexture==nil,'hiding did not restore the dynamic texture')
UI_Character_Skin.SetActive(exactLobby,true)
assert(firstFrameGraphic.OverrideTexture==firstFramePage.texture,'reactivation waited for the polling timer')
local updateCalls=0
local update=MJCustomPortrait.update
MJCustomPortrait.update=function(...)updateCalls=updateCalls+1;return update(...)end
UI_Character_Skin.SetSkin(exactLobby,{skinId=401701,accountId=10,complete=function(...)
 assert(select('#',...)==1 and select(1,...)==true,'cancellation arguments changed')
end})
exactLobby.pending(true);assert(updateCalls==0,'canceled load applied a replacement')
MJCustomPortrait.update=update
MJCustomPortrait.close()
-- Avatar sprites share the collection transaction and follow asynchronous
-- native image completion without changing other UI image requests.
MJCustomHeadTargets=nil
local requests={}
ImageMgr={SetTextureAsync=function(image,path,tag,complete)
 requests[#requests+1]={image=image,path=path,complete=complete}
 return 'request',nil,7
end}
SkinTool={GetSkinIconDir=function(id)if id==401701 then return 'legacy/test' end;return 'deco/character/'..tostring(id)end}
Tools={SetHeadImg=function(image,id,kind)
 image.sprite={placeholder=true}
 ImageMgr.SetTextureAsync(image,SkinTool.GetSkinIconDir(id)..'/'..kind..'/'..kind,nil,function(...)image.callbackSprite=image.sprite;image.callbackArgs=select('#',...)end)
 return 'head',nil,9
end}
local avatarSource=MJCustomPortraitSource:gsub('local mods=.-\nlocal loaded','local mods={{character=200017,skin=401701,index=1,options={},smallhead={index=2},bighead={index=3}}}\nlocal loaded',1)
reload(avatarSource)
local small={sprite={base=true}}
local x,y,z=Tools.SetHeadImg(small,401701,'smallhead')
local smallEntry=MJCustomPortrait.mods[1].smallhead
assert(x=='head' and y==nil and z==9,'avatar hook changed return values')
assert(small.sprite==smallEntry.sprite,'small avatar waited for polling')
local nativeSmall={nativeSmall=true}
local req=requests[#requests];small.sprite=nativeSmall;req.complete(1,nil,3)
assert(small.sprite==smallEntry.sprite and small.callbackSprite==smallEntry.sprite and small.callbackArgs==3,'async avatar completed before replacement or lost callback arguments')
local large={sprite={base=true}}
local observed
x,y,z=ImageMgr.SetTextureAsync(large,'legacy/test/bighead/bighead',nil,function(...)observed=large.sprite;assert(select('#',...)==3)end)
assert(x=='request' and y==nil and z==7,'image hook changed return values')
local nativeLarge={nativeLarge=true};large.sprite=nativeLarge;requests[#requests].complete(1,nil,3)
assert(observed==MJCustomPortrait.mods[1].bighead.sprite and large.sprite~=small.sprite,'direct large avatar request used wrong replacement')
local firstSmall=small.sprite
reload(avatarSource)
assert(firstSmall.destroyed and small.sprite==MJCustomPortrait.mods[1].smallhead.sprite,'F10 retained a released avatar sprite')
MJCustomPortrait.close()
assert(small.sprite==nativeSmall and large.sprite==nativeLarge,'F11 did not restore latest async native sprites')
reload(avatarSource)
ImageMgr.SetTextureAsync(small,'ui/other/icon',nil,nil)
assert(small.sprite==nativeSmall and not MJCustomHeadTargets[small],'recycled non-avatar image retained replacement')
Tools.SetHeadImg(small,401801,'smallhead');local otherNative={other=true};small.sprite=otherNative;requests[#requests].complete()
assert(small.sprite==otherNative,'unconfigured skin avatar was replaced')
local previous=MJCustomPortrait
local nativePixels=_MJCustomPixels
_MJCustomPixels=function(index)if index==99 then return 'invalid' end;return nativePixels(index)end
local invalidAvatar=avatarSource:gsub('smallhead={index=2}','smallhead={index=99}',1)
assert(not pcall(reload,invalidAvatar) and MJCustomPortrait==previous and previous.enabled,'bad avatar discarded the previous collection')
_MJCustomPixels=nativePixels
MJCustomPortrait.close()
_MJCustomPixels=oldpixels;typeof=oldtypeof
for k,v in pairs(saved)do _G[k]=v end
for _,k in ipairs({'Tools','SkinTool','UI_DesktopInfo','UI_FriendRoom','UI_LiaosheMain','UI_LiaoSheChangeSkin','UI_GameEnd','DesktopMgr','MJCustomPortrait','UI_Character_Skin','ImageMgr','MJCustomHeadTargets'})do if saved[k]==nil then _G[k]=nil end end
