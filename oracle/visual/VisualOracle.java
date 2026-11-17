import java.nio.file.*;
import java.io.*;
import java.security.*;
import java.util.*;

public class VisualOracle {
    static void environment(DataOutputStream out,Environment e)throws Exception {
        out.writeInt(e.screenColorRgb);out.writeFloat(e.lightModelAmbient);out.writeFloat(e.light0Diffuse);out.writeFloat(e.light1Diffuse);
        out.writeInt(e.lightX);out.writeInt(e.lightY);out.writeInt(e.lightZ);out.writeInt(e.fogColorRgb);out.writeInt(e.fogDepth);
    }
    static void parameters(DataOutputStream out,int color,float ambient,float diffuse0,float diffuse1,int fogColor,int fogDepth)throws Exception {
        out.writeInt(color);out.writeFloat(ambient);out.writeFloat(diffuse0);out.writeFloat(diffuse1);out.writeInt(fogColor);out.writeInt(fogDepth);
    }
    static void atmosphere(DataOutputStream out)throws Exception {
        parameters(out,FogManager.currentScreenColor,FogManager.currentAmbient,FogManager.currentLight0Diffuse,FogManager.currentLight1Diffuse,FogManager.currentFogColor,FogManager.currentFogDepth);
        parameters(out,FogManager.targetScreenColor,FogManager.targetAmbient,FogManager.targetLight0Diffuse,FogManager.targetLight1Diffuse,FogManager.targetFogColor,FogManager.targetFogDepth);
        parameters(out,FogManager.prevScreenColor,FogManager.prevAmbient,FogManager.prevLight0Diffuse,FogManager.prevLight1Diffuse,FogManager.prevFogColor,FogManager.prevFogDepth);
        out.writeInt(FogManager.fadeProgress);out.writeInt(FogManager.instantScreenFade?1:0);
        for(float[] row:GlRenderer.gl.lights)for(float v:row)out.writeFloat(v);
        for(float v:GlRenderer.gl.fog)out.writeFloat(v);
        for(float v:GlRenderer.gl.range)out.writeFloat(v);
    }
    static void rotate(GlModel model,int quadrant,boolean normals){
        switch(quadrant&3){case 1:if(normals)model.rotateCounterClockwiseAll();else model.rotateCounterClockwise();break;case 2:if(normals)model.rotate180All();else model.rotate180();break;case 3:if(normals)model.rotateClockwiseAll();else model.rotateClockwise();break;}
    }
    static String digest(byte[] bytes)throws Exception{return HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(bytes));}
    static void canonical(DataOutputStream out,RawModel m)throws Exception {
        out.writeInt(m.vertexCount);out.writeInt(m.triangleCount);
        for(int i=0;i<m.vertexCount;i++){out.writeInt(m.vertexX[i]);out.writeInt(m.vertexY[i]);out.writeInt(m.vertexZ[i]);}
        for(int i=0;i<m.triangleCount;i++){out.writeInt(m.triangleVertexA[i]);out.writeInt(m.triangleVertexB[i]);out.writeInt(m.triangleVertexC[i]);}
        for(int i=0;i<m.triangleCount;i++)out.writeShort(m.triangleColors[i]);
        for(int i=0;i<m.triangleCount;i++)out.writeByte(m.triangleAlpha==null?0:m.triangleAlpha[i]);
        out.writeInt(m.vertexBones==null?0:m.vertexCount);if(m.vertexBones!=null)for(int i=0;i<m.vertexCount;i++)out.writeByte(m.vertexBones[i]);
        out.writeInt(m.triangleBones==null?0:m.triangleBones.length);if(m.triangleBones!=null)for(int v:m.triangleBones)out.writeByte(v);
        for(int i=0;i<m.triangleCount;i++)out.writeInt(m.triangleTextures==null?-1:m.triangleTextures[i]);
    }
    public static void main(String[] args)throws Exception {
        Path dir=Path.of(args[0]);
        Rasteriser.textureProvider=new VisualMaterials(Files.readAllBytes(dir.resolve("materials.bin")));
        IdkTypeList.modelsArchive=new Js5(dir);
        byte[] noiseBytes=Files.readAllBytes(dir.resolve("light-noise.bin"));Light.NOISE=new int[2048];java.nio.ByteBuffer noiseBuffer=java.nio.ByteBuffer.wrap(noiseBytes);for(int i=0;i<2048;i++)Light.NOISE[i]=noiseBuffer.getInt();
        for(String line:Files.readAllLines(dir.resolve("requests.tsv"))){
            String[] p=line.split("\t");byte[] data=Files.readAllBytes(dir.resolve(p[1]));
            if(p[0].equals("locchanges")){
                SceneGraph.objects.clear();SceneGraph.unready.clear();SceneGraph.events.clear();ChangeLocRequest.queue=new LinkedList();
                SceneGraph.addLocModel(100,50,0,0,50,0,0);
                ByteArrayOutputStream bytes=new ByteArrayOutputStream();DataOutputStream out=new DataOutputStream(bytes);
                for(String command:p[2].split(";")){
                    SceneGraph.events.clear();int[] c=Arrays.stream(command.split(",")).mapToInt(Integer::parseInt).toArray();
                    if(c[0]==0)ChangeLocRequest.push(c[1],c[3],c[7],c[2],c[9],c[5],c[4],c[6],c[8]);
                    else if(c[0]==1)ChangeLocRequest.loop();
                    else if(c[0]==2){if(c[2]==0)SceneGraph.unready.add(c[1]);else SceneGraph.unready.remove(c[1]);}
                    else if(c[0]==3)ChangeLocRequest.flush();
                    out.writeInt(SceneGraph.objects.size());for(var entry:SceneGraph.objects.entrySet()){var k=entry.getKey();long key=entry.getValue();for(int v:new int[]{k.plane,k.x,k.y,k.layer,(int)(key>>>32)&Integer.MAX_VALUE,(int)(key>>14)&31,(int)(key>>20)&3})out.writeInt(v);}
                    int n=0;for(Node node=ChangeLocRequest.queue.head();node!=null;node=ChangeLocRequest.queue.next())n++;out.writeInt(n);
                    for(Node node=ChangeLocRequest.queue.head();node!=null;node=ChangeLocRequest.queue.next()){ChangeLocRequest r=(ChangeLocRequest)node;for(int v:new int[]{r.level,r.x,r.y,r.layer,r.originalId,r.originalShape,r.originalAngle,r.newId,r.newShape,r.newAngle,r.setLoops,r.resetLoops})out.writeInt(v);}
                    out.writeInt(SceneGraph.events.size());for(int[] event:SceneGraph.events)for(int v:event)out.writeInt(v);
                }
                System.out.println("locchanges\t"+p[1]+"\t"+digest(bytes.toByteArray()));
            }else if(p[0].equals("locinfo")||p[0].equals("locmenu")){
                LocType loc=LocType.clipping(data);loc.postDecode();loc.id=12345;
                ByteArrayOutputStream bytes=new ByteArrayOutputStream();DataOutputStream out=new DataOutputStream(bytes);
                if(p[0].equals("locinfo")){
                    byte[] name=loc.name.value.getBytes(java.nio.charset.StandardCharsets.UTF_8);out.writeInt(name.length);out.write(name);
                    for(int v:new int[]{loc.width,loc.length,loc.blocksides,loc.interactable,loc.cursor1Op,loc.cursor1,loc.cursor2Op,loc.cursor2})out.writeInt(v);
                    for(JagString op:loc.ops){out.writeInt(op==null?-1:op.value.getBytes(java.nio.charset.StandardCharsets.UTF_8).length);if(op!=null)out.write(op.value.getBytes(java.nio.charset.StandardCharsets.UTF_8));}
                }else{
                    MenuOracle.size=0;Cs1ScriptRunner.isMenuOpen=false;
                    MenuOracle.location(loc,((long)12345<<32)|0x40000000L|52|(53<<7)|(10<<14),52,53);MenuOracle.sort();
                    out.writeInt(MenuOracle.size);
                    for(int i=0;i<MenuOracle.size;i++){for(JagString text:new JagString[]{MenuOracle.ops[i],MenuOracle.opBases[i]}){byte[] value=text.value.getBytes(java.nio.charset.StandardCharsets.UTF_8);out.writeInt(value.length);out.write(value);}out.writeInt(MenuOracle.actions[i]);out.writeLong(MenuOracle.keys[i]);out.writeInt(MenuOracle.intArgs1[i]);out.writeInt(MenuOracle.intArgs2[i]);out.writeInt(MenuOracle.cursors[i]);}
                }
                System.out.println(p[0]+"\t"+p[1]+"\t"+digest(bytes.toByteArray()));
            }else if(p[0].equals("locaction")){
                Protocol.outboundBuffer=new OutBuffer();Camera.originX=Camera.originY=0;
                int action=Integer.parseInt(p[2]);int location=Integer.parseInt(p[3]);long key=action==1004||action==3004?location:((long)location<<32)|0x40000000;
                MenuOracle.action(action,key,Integer.parseInt(p[4]),Integer.parseInt(p[5]));
                System.out.println("locaction\t"+p[1]+"\t"+digest(Protocol.outboundBuffer.out.toByteArray()));
            }else if(p[0].startsWith("npc")){
                NpcTypeList.modelsArchive=new Js5(dir);
                NpcDefinitionOracle d=new NpcDefinitionOracle();d.decode(new Buffer(data));
                BasType bas=new BasType();bas.decode(new Buffer(Files.readAllBytes(dir.resolve(p[2]))));
                if(d.multiNpcs!=null||d.recol_p!=null||bas.modelRotateTranslate!=null)throw new IllegalStateException("NPC corpus exceeds declared scope");
                ByteArrayOutputStream bytes=new ByteArrayOutputStream();DataOutputStream out=new DataOutputStream(bytes);
                if(p[0].equals("npcdecode")){
                    byte[] name=d.name.value.getBytes(java.nio.charset.StandardCharsets.ISO_8859_1);out.writeInt(name.length);out.write(name);
                    for(int v:new int[]{d.size,d.bastypeid,d.resizeX,d.resizeY,d.ambient+64,d.contrast+850,d.rotationspeed,d.rotationflag?1:0,d.hasshadow?1:0,d.shadowcolor1&65535,d.shadowcolor2&65535,d.shadowcolormodifier1&255,d.shadowcolormodifier2&255})out.writeInt(v);
                    out.writeInt(d.modelIndices.length);for(int v:d.modelIndices)out.writeInt(v);
                    for(short[][] pairs:new short[][][]{new short[][]{d.recol_s,d.recol_d},new short[][]{d.retex_s,d.retex_d}}){int n=pairs[0]==null?0:pairs[0].length;out.writeInt(n);for(int i=0;i<n;i++){out.writeShort(pairs[0][i]);out.writeShort(pairs[1][i]);}}
                    int offsets=0;if(d.modeloffsets!=null)for(int[] row:d.modeloffsets)if(row!=null)offsets++;out.writeInt(offsets);if(d.modeloffsets!=null)for(int i=0;i<d.modeloffsets.length;i++)if(d.modeloffsets[i]!=null){out.writeInt(i);for(int v:d.modeloffsets[i])out.writeInt(v);}
                    for(int v:new int[]{bas.idleAnimationId,bas.walkAnimation,bas.runAnimationId,bas.standingCCWTurn,bas.standingCWTurn,bas.walkFullTurnAnimationId,bas.walkCCWTurnAnimationId,bas.walkCWTurnAnimationId,bas.terrainAlignWidth,bas.terrainAlignDepth})out.writeInt(v);
                }else{
                    RawModel m=d.assemble();
                    if(p[0].equals("npcmodel")){canonical(out,m);}else{
                        SeqType seq=new SeqType();seq.decode(new Buffer(Files.readAllBytes(dir.resolve(p[3]))));
                        AnimBase base=new AnimBase(Integer.parseInt(p[4]),Files.readAllBytes(dir.resolve(p[5])));
                        AnimFrame a=new AnimFrame(Files.readAllBytes(dir.resolve(p[6])),base),b=new AnimFrame(Files.readAllBytes(dir.resolve(p[7])),base);
                        m.triangleSources=new short[m.triangleCount];for(int i=0;i<m.triangleCount;i++)m.triangleSources[i]=(short)i;
                        GlModel g=new GlModel(m,d.ambient+64,d.contrast+850,true);g.createBones();
                        g.applyAnimation(new AnimFrameset(a),0,Boolean.parseBoolean(p[10])?new AnimFrameset(b):null,0,Integer.parseInt(p[8]),Integer.parseInt(p[9]),seq.hasModelTransforms);
                        g.resize(d.resizeX,d.resizeY,d.resizeX);
                        for(int i=0;i<m.vertexCount;i++){out.writeInt(g.vertexX[i]);out.writeInt(g.vertexY[i]);out.writeInt(g.vertexZ[i]);}
                        int[] lookup=new int[m.triangleCount];Arrays.fill(lookup,-1);for(int i=0;i<g.triangleCount;i++)lookup[g.triangleSources[i]&65535]=i;
                        for(int index:lookup)if(index>=0)for(int v:new int[]{g.triangleVertexA[index]&65535,g.triangleVertexB[index]&65535,g.triangleVertexC[index]&65535}){out.writeInt(g.normalX[v]);out.writeInt(g.normalY[v]);out.writeInt(g.normalZ[v]);out.writeInt(g.normalMagnitude[v]);}
                        for(int index:lookup)if(index>=0){out.writeShort(g.triangleColors[index]);out.writeByte(g.triangleAlpha[index]);}
                        CharacterShadowOracle.animation=a;SeqType shadowSeq=new SeqType();shadowSeq.frames=new int[]{0};
                        GlModel s=CharacterShadowOracle.getOrCreateShadow(d.shadowcolormodifier1&255,seq.stretches,shadowSeq,6976,d.shadowcolor1&65535,6464,d.shadowcolor2&65535,d.size,g,Integer.parseInt(p[11]),0,0,d.shadowcolormodifier2&255);
                        for(int i=0;i<s.vertexCount;i++){out.writeInt(s.vertexX[i]);out.writeInt(s.vertexY[i]);out.writeInt(s.vertexZ[i]);}for(int i=0;i<s.triangleCount;i++){out.writeShort(s.triangleColors[i]);out.writeByte(s.triangleAlpha[i]);}
                    }
                }
                System.out.println(p[0]+"\t"+p[1]+"\t"+digest(bytes.toByteArray()));
            } else if(p[0].equals("depthbias")){
                float near=Float.parseFloat(p[3]),far=Float.parseFloat(p[4]);GlRenderer.projectionZScale=-(far+near)/(far-near);GlRenderer.projectionZOffset=-(near*2.0F*far)/(far-near);GlRenderer.projectionDistance=Float.NaN;GlRenderer.setDepthBias(Float.parseFloat(p[5]),Float.parseFloat(p[6]));ByteArrayOutputStream bytes=new ByteArrayOutputStream();DataOutputStream out=new DataOutputStream(bytes);out.writeFloat(GlRenderer.matrix[10]);out.writeFloat(GlRenderer.matrix[14]);System.out.println("depthbias\t"+p[2]+"\t"+digest(bytes.toByteArray()));
            } else if(p[0].equals("lightplace")){
                Light light=new Light(new Buffer(data));PointPlacementOracle.place(light,Integer.parseInt(p[2]),Integer.parseInt(p[3]),Integer.parseInt(p[4]));ByteArrayOutputStream bytes=new ByteArrayOutputStream();DataOutputStream out=new DataOutputStream(bytes);out.writeInt(PointPlacementOracle.added?1:0);if(PointPlacementOracle.added){out.writeInt(light.x);out.writeInt(light.y);out.writeInt(light.z);}System.out.println("lightplace\t"+p[1]+"\t"+digest(bytes.toByteArray()));
            } else if(p[0].equals("lightgrid")){
                String[] files=p[4].split("\\|");int pattern=Integer.parseInt(p[3]);LightGridOracle.initLightGrid();LightGridOracle.lights=new Light[255];LightGridOracle.lightCount=0;
                java.io.PrintStream originalOutput=System.out;
                try(var silent=new java.io.PrintStream(java.io.OutputStream.nullOutputStream())){System.setOut(silent);
                    for(int i=0;i<files.length;i++){Light light=new Light(new Buffer(Files.readAllBytes(dir.resolve(files[i]))));int center=pattern==1?0:pattern==2?103:pattern==4&&i<255?1:50;light.x=center*128+(i%3)*32;light.y=center*128+(i%5)*40;LightGridOracle.addLight(light);}
                }finally{System.setOut(originalOutput);}
                LightGridOracle.buildLightGrid();ByteArrayOutputStream bytes=new ByteArrayOutputStream();DataOutputStream out=new DataOutputStream(bytes);for(int plane=0;plane<4;plane++)for(int x=0;x<104;x++)for(int y=0;y<104;y++)out.writeInt(LightGridOracle.lightGrid[plane][x][y]);System.out.println("lightgrid\t"+p[2]+"\t"+digest(bytes.toByteArray()));
            } else if(p[0].equals("shadow_stamp")){
                SoftwareIndexedSprite sprite=new SoftwareIndexedSprite(17,23,0);for(int i=0;i<sprite.pixels.length;i++)sprite.pixels[i]=(byte)(i%3==0?0:1);sprite.xOffset=-7;sprite.yOffset=-12;SoftwareIndexedSprite dest=new SoftwareIndexedSprite(258,258,0);dest.pixels=data.clone();int pattern=Integer.parseInt(p[2]);int x=new int[]{1,257,0,100,128,250,-10,50}[pattern],y=new int[]{1,257,130,0,128,250,90,256}[pattern];for(int i=0;i<260;i++)ShadowStamp.addShadowSprite(sprite,dest,x,y);for(int i=0;i<3;i++)ShadowStamp.removeShadowSprite(sprite,dest,x,y);System.out.println("shadow_stamp\t"+pattern+"\t"+digest(dest.pixels));
            } else if(p[0].equals("light")){
                Buffer buffer=new Buffer(data);Light light=new Light(buffer);if(light.animationPreset==31){buffer.g2();light.setFlickerParams(5,3000,4096,-1000);}ByteArrayOutputStream bytes=new ByteArrayOutputStream();DataOutputStream out=new DataOutputStream(bytes);
                for(int value:new int[]{light.level,light.extendsDown?1:0,light.extendsUp?1:0,light.x,light.y,light.z,light.radius,light.color,light.animationPreset,light.animationPhase,light.flickerType,light.flickerSpeed,light.alphaMin,light.alphaMax})out.writeInt(value);out.writeFloat(light.quadraticAttenuation);for(short span:light.spanProfile)out.writeShort(span);
                for(boolean disable:new boolean[]{false,true})for(int tick:new int[]{0,1,2,3,7,15,31,50,100,199,200,511,1023,2048,-1,-50,Integer.MAX_VALUE,Integer.MIN_VALUE}){light.updateAnimation(disable,tick);out.writeFloat(light.alpha);for(float channel:light.diffuse)out.writeFloat(channel);}
                for(int i=0;i<32;i++){LightMesh mesh=new LightMesh();int x=light.x+(i%8-3)*70,y=light.y+(i%5-2)*50,z=light.z+(i%3-1)*140;if(i==31){x=light.x;y=light.y;z=light.z;}int vertex=mesh.addVertex(light,x,y,z,0.0F,-1.0F,0.0F);out.writeByte(mesh.colorR[vertex]);out.writeByte(mesh.colorG[vertex]);out.writeByte(mesh.colorB[vertex]);}
                System.out.println("light\t"+p[1]+"\t"+digest(bytes.toByteArray()));
            } else if(p[0].equals("shadow")){
                GlModel g=new GlModel(new RawModel(data),64,768,false);FogManager.lightX=Integer.parseInt(p[3]);FogManager.lightZ=Integer.parseInt(p[4]);SoftwareIndexedSprite sprite=g.projectShadow(null);
                ByteArrayOutputStream bytes=new ByteArrayOutputStream();DataOutputStream out=new DataOutputStream(bytes);if(sprite!=null){out.writeInt(sprite.xOffset);out.writeInt(sprite.yOffset);out.writeInt(sprite.width);out.writeInt(sprite.height);out.write(sprite.pixels);}System.out.println("shadow\t"+p[2]+"\t"+digest(bytes.toByteArray()));
            } else if(p[0].equals("shadow_filter")){
                SoftwareIndexedSprite sprite=new SoftwareIndexedSprite(258,258,0);sprite.pixels=data;ByteArrayOutputStream bytes=new ByteArrayOutputStream();for(int y=0;y<2;y++)for(int x=0;x<2;x++){ShadowFilter filter=new ShadowFilter();filter.updateShadowTexture(sprite,x,y);bytes.write(ShadowFilter.pixels);}System.out.println("shadow_filter\t"+p[1]+"\t"+digest(bytes.toByteArray()));
            } else if(p[0].equals("clipping")){
                LocType loc=LocType.clipping(data);ByteArrayOutputStream bytes=new ByteArrayOutputStream();DataOutputStream out=new DataOutputStream(bytes);for(int value:new int[]{loc.width,loc.length,loc.blockwalk,loc.blockrange?1:0,loc.breakroutefinding?1:0})out.writeInt(value);System.out.println("clipping\t"+p[1]+"\t"+digest(bytes.toByteArray()));
            } else if(p[0].equals("route")){
                CollisionMap map=new CollisionMap(104,104);DataInputStream in=new DataInputStream(new ByteArrayInputStream(data));for(int x=0;x<104;x++)for(int y=0;y<104;y++)map.flags[x][y]=in.readInt();PathFinder.collisionMaps[0]=map;Player.plane=0;
                Camera.originX=Integer.parseInt(p[9]);Camera.originY=Integer.parseInt(p[10]);Keyboard.pressedKeys[0]=p[8].equals("1");Protocol.outboundBuffer=new OutBuffer();LoginManager.mapFlagX=LoginManager.mapFlagY=-1;ClientProt.lastPathLength=0;
                boolean found=PathFinder.findPathN(Integer.parseInt(p[5]),p.length>11?Integer.parseInt(p[15]):0,Integer.parseInt(p[3]),Integer.parseInt(p[6]),p.length>11?2:0,p.length>11?Integer.parseInt(p[12]):0,p.length>11?Integer.parseInt(p[14]):0,p[7].equals("1"),p.length>11?Integer.parseInt(p[13]):0,Integer.parseInt(p[4]),p.length>11?Integer.parseInt(p[11]):0);
                ByteArrayOutputStream bytes=new ByteArrayOutputStream();DataOutputStream out=new DataOutputStream(bytes);out.writeInt(found?1:0);out.writeInt(PathFinder.approximateDestination);byte[] packet=Protocol.outboundBuffer.out.toByteArray();out.writeInt(packet.length);out.write(packet);out.writeInt(LoginManager.mapFlagX);out.writeInt(LoginManager.mapFlagY);out.writeInt(ClientProt.lastPathLength);for(int i=0;i<ClientProt.lastPathLength;i++){out.writeInt(PathFinder.queueX[i]);out.writeInt(PathFinder.queueY[i]);}
                System.out.println("route\t"+p[2]+"\t"+digest(bytes.toByteArray()));
            } else if(p[0].equals("movement")){
                BasType bas=new BasType();bas.decode(new Buffer(data));SeqTypeList.sequences.clear();try(var paths=Files.list(dir)){for(Path path:paths.filter(v->v.getFileName().toString().matches("seq_\\d+\\.bin")).toList()){String name=path.getFileName().toString();SeqType seq=new SeqType();seq.decode(new Buffer(Files.readAllBytes(path)));SeqTypeList.sequences.put(Integer.parseInt(name.substring(4,name.length()-4)),seq);}}
                Player player=new Player();player.bas=bas;player.xFine=player.yFine=50*128+64;player.movementQueueX[0]=player.movementQueueY[0]=50;player.currentAngle=Integer.parseInt(p[3]);PlayerList.self=player;
                if(p.length>6){player.oracleSize=Integer.parseInt(p[6]);player.xFine=player.yFine=50*128+player.oracleSize*64;}
                int[][] commands=Arrays.stream(p[4].split(";")).map(row->Arrays.stream(row.split(",")).mapToInt(Integer::parseInt).toArray()).toArray(int[][]::new);
                ByteArrayOutputStream bytes=new ByteArrayOutputStream();DataOutputStream out=new DataOutputStream(bytes);
                for(int tick=0;tick<Integer.parseInt(p[5]);tick++){for(int[] cmd:commands)if(cmd[0]==tick)player.move(cmd[2],cmd[1]);SoundPlayer.events.clear();NpcList.processMovement(player);NpcList.processFacing(player);NpcList.advanceMovementAnimation(player);for(int value:new int[]{player.xFine,player.yFine,player.currentAngle,player.targetAngle,player.movementSeqId,player.standingTurnCounter,player.movementSeqFrame,player.movementSeqNextFrame,player.movementSeqDelayClock,player.movementQueueSize})out.writeInt(value);for(int i=player.movementQueueSize-1;i>=0;i--){out.writeInt(player.movementQueueX[i]);out.writeInt(player.movementQueueY[i]);out.writeInt(player.movementQueueSpeed[i]);}out.writeInt(SoundPlayer.events.size());for(int sound:SoundPlayer.events)out.writeInt(sound);}
                System.out.println("movement\t"+p[2]+"\t"+digest(bytes.toByteArray()));
            } else if(p[0].equals("environment")){
                Environment e=data.length==0?new Environment():new Environment(new Buffer(data));ByteArrayOutputStream bytes=new ByteArrayOutputStream();environment(new DataOutputStream(bytes),e);System.out.println("environment\t"+p[1]+"\t"+digest(bytes.toByteArray()));
            } else if(p[0].equals("atmosphere")){
                FogManager.resetForOracle();GlRenderer.gl=new GL2();GlobalConfig.VIEW_DISTANCE=Integer.parseInt(p[5]);GlobalConfig.VIEW_FADE_DISTANCE=Float.parseFloat(p[6]);
                String[] files=p[3].split("\\|");Environment[] env=new Environment[files.length];for(int i=0;i<files.length;i++)env[i]=new Environment(new Buffer(Files.readAllBytes(dir.resolve(files[i]))));
                ByteArrayOutputStream bytes=new ByteArrayOutputStream();DataOutputStream out=new DataOutputStream(bytes);
                for(String row:p[4].split(";")){int[] step=Arrays.stream(row.split(",")).mapToInt(Integer::parseInt).toArray();FogManager.chunksAtmosphere[0][0]=env[step[0]];Preferences.fogEnabled=step[3]!=0;if(step[4]!=0)FogManager.instantScreenFade=true;FogManager.updateAtmosphere(step[1],0,step[2],0);atmosphere(out);}
                if(p[2].equals(System.getProperty("visual.dumpAtmosphere")))Files.write(dir.resolve(p[2]+".atmosphere"),bytes.toByteArray());
                System.out.println("atmosphere\t"+p[2]+"\t"+digest(bytes.toByteArray()));
            } else if(p[0].equals("locclock")){
                SeqType seq=new SeqType();seq.decode(new Buffer(data));
                double[] random=p[4].equals("none")?null:Arrays.stream(p[4].split(",")).mapToDouble(Double::parseDouble).toArray();
                int start=Integer.parseInt(p[3]);LocClock clock=new LocClock(seq,start,random);
                ByteArrayOutputStream bytes=new ByteArrayOutputStream();DataOutputStream out=new DataOutputStream(bytes);
                for(String tick:p[5].split(",")){client.loop=start+Integer.parseInt(tick);SoundPlayer.events.clear();clock.advanceAnimation(0,0);out.writeInt(clock.seq==null?0:1);out.writeInt(clock.seqFrame);out.writeInt(clock.seqNextFrame);out.writeInt(clock.seqFrameProgress);out.writeInt(clock.seqStartLoop);out.writeInt(SoundPlayer.events.size());for(int sound:SoundPlayer.events)out.writeInt(sound);}
                System.out.println("locclock\t"+p[2]+"\t"+digest(bytes.toByteArray()));
            } else if(p[0].equals("model")){
                RawModel m=new RawModel(data);ByteArrayOutputStream bytes=new ByteArrayOutputStream();canonical(new DataOutputStream(bytes),m);
                System.out.println("model\t"+p[1]+"\t"+digest(bytes.toByteArray()));
            } else if(p[0].equals("underlay")||p[0].equals("overlay")) {
                ByteArrayOutputStream bytes=new ByteArrayOutputStream();DataOutputStream out=new DataOutputStream(bytes);
                if(p[0].equals("underlay")){FluType t=new FluType();t.decode(0,new Buffer(data));for(int v:new int[]{t.weightedHue,t.saturation,t.lightness,t.chroma,t.texture,t.textureScale})out.writeInt(v);}
                else{FloType t=new FloType();t.decode(new Buffer(data),0);for(int v:new int[]{t.baseColor,t.texture,t.textureScale,t.blendTexture?1:0,t.waterColor,t.waterOpacity})out.writeInt(v);}
                System.out.println(p[0]+"\t"+p[1]+"\t"+digest(bytes.toByteArray()));
            } else if(p[0].equals("floorblend")) {
                FluTypeList.types=new FluType[256];for(int i=0;i<256;i++){Path path=dir.resolve("underlay_"+i+".bin");if(Files.exists(path)){FluType t=new FluType();t.decode(i,new Buffer(Files.readAllBytes(path)));FluTypeList.types[i]=t;}}
                byte[][] tiles=new byte[104][104];for(int x=0;x<104;x++)System.arraycopy(data,x*104,tiles[x],0,104);int[][] colors=FloorBlendOracle.blend(tiles);
                ByteArrayOutputStream bytes=new ByteArrayOutputStream();DataOutputStream out=new DataOutputStream(bytes);for(int[] row:colors)for(int color:row)out.writeShort(color);System.out.println("floorblend\t"+p[1]+"\t"+digest(bytes.toByteArray()));
            } else if(p[0].equals("appearance")) {
                int[] colors=Arrays.stream(p[3].split(",")).mapToInt(Integer::parseInt).toArray();int[] ids=Arrays.stream(p[4].split(",")).mapToInt(Integer::parseInt).toArray();IdkType[] kits=new IdkType[ids.length];for(int i=0;i<ids.length;i++){kits[i]=new IdkType();kits[i].decode(new Buffer(Files.readAllBytes(dir.resolve("kit_"+ids[i]+".bin"))));}
                RawModel m=AppearanceOracle.assemble(kits,colors);ByteArrayOutputStream bytes=new ByteArrayOutputStream();canonical(new DataOutputStream(bytes),m);System.out.println("appearance\t"+p[2]+"\t"+digest(bytes.toByteArray()));
            } else if(p[0].equals("surface")) {
                RawModel m=new RawModel(data);m.triangleSources=new short[m.triangleCount];for(int i=0;i<m.triangleCount;i++)m.triangleSources[i]=(short)i;
                GlModel g=new GlModel(m,64,768,false);int[] lookup=new int[m.triangleCount];Arrays.fill(lookup,-1);for(int i=0;i<g.triangleCount;i++)lookup[g.triangleSources[i]&65535]=i;
                ByteArrayOutputStream bytes=new ByteArrayOutputStream();DataOutputStream out=new DataOutputStream(bytes);out.writeInt(m.triangleCount);
                for(int i=0;i<m.triangleCount;i++){
                    int index=lookup[i];out.writeBoolean(index>=0);if(index<0)continue;
                    for(int v:new int[]{g.triangleVertexA[index]&65535,g.triangleVertexB[index]&65535,g.triangleVertexC[index]&65535}){
                        out.writeInt(g.normalX[v]);out.writeInt(g.normalY[v]);out.writeInt(g.normalZ[v]);out.writeInt(g.normalMagnitude[v]);out.writeFloat(g.vertexS[v]);out.writeFloat(g.vertexT[v]);
                    }
                    out.writeInt(GlModel.packColorToRGBA(g.triangleColors[index]&65535,g.triangleTextures[index],64,g.triangleAlpha[index]));
                }
                for(int i=0;i<g.triangleCount;i++)out.writeShort(g.triangleSources[i]);
                Files.write(dir.resolve(p[1]+".surface"),bytes.toByteArray());System.out.println("surface\t"+p[1]+"\t"+digest(bytes.toByteArray()));
            } else if(p[0].equals("sound")) {
                PcmSound sound=new SynthSound(new Buffer(data)).toPcmSound();
                Files.write(dir.resolve(p[1]+".pcm"),sound.samples);
                System.out.println("sound\t"+p[1]+"\t"+digest(sound.samples)+"\t"+sound.samples.length+"\t"+sound.start+"\t"+sound.end);
            } else if(p[0].equals("pose")||p[0].equals("glpose")||p[0].equals("locpose")||p[0].equals("character")) {
                RawModel m=new RawModel(data);
                if(p.length>9){String[] files=p[9].split("\\|");if(files.length>1||!p[0].equals("locpose")){RawModel[] parts=new RawModel[files.length];for(int i=0;i<parts.length;i++)parts[i]=new RawModel(Files.readAllBytes(dir.resolve(files[i])));m=new RawModel(parts,parts.length);}}
                int id=Integer.parseInt(p[2]);AnimBase base=new AnimBase(id,Files.readAllBytes(dir.resolve(p[3])));
                AnimFrame a=new AnimFrame(Files.readAllBytes(dir.resolve(p[4])),base);AnimFrame b=new AnimFrame(Files.readAllBytes(dir.resolve(p[5])),base);
                ByteArrayOutputStream bytes=new ByteArrayOutputStream();
                if(p[0].equals("glpose")||p[0].equals("locpose")||p[0].equals("character")){
                    m.triangleSources=new short[m.triangleCount];for(int i=0;i<m.triangleCount;i++)m.triangleSources[i]=(short)i;
                    GlModel g=new GlModel(m,64,850,true);g.createBones();
                    boolean loc=p[0].equals("locpose"),normals=!loc||Boolean.parseBoolean(p[11]);int rotation=loc?Integer.parseInt(p[10]):0;
                    if(loc){rotate(g,rotation,true);int[] scale=Arrays.stream(p[12].split(",")).mapToInt(Integer::parseInt).toArray();int[] offset=Arrays.stream(p[13].split(",")).mapToInt(Integer::parseInt).toArray();g.resize(scale[0],scale[1],scale[2]);g.translate(offset[0],offset[1],offset[2]);rotate(g,-rotation,normals);}
                    g.applyAnimation(new AnimFrameset(a),0,new AnimFrameset(b),0,Integer.parseInt(p[6]),Integer.parseInt(p[7]),normals);
                    if(loc)rotate(g,rotation,normals);
                    DataOutputStream out=new DataOutputStream(bytes);for(int i=0;i<m.vertexCount;i++){out.writeInt(g.vertexX[i]);out.writeInt(g.vertexY[i]);out.writeInt(g.vertexZ[i]);}
                    int[] lookup=new int[m.triangleCount];Arrays.fill(lookup,-1);for(int i=0;i<g.triangleCount;i++)lookup[g.triangleSources[i]&65535]=i;
                    for(int i=0;i<m.triangleCount;i++){int index=lookup[i];if(index<0)continue;for(int v:new int[]{g.triangleVertexA[index]&65535,g.triangleVertexB[index]&65535,g.triangleVertexC[index]&65535}){out.writeInt(g.normalX[v]);out.writeInt(g.normalY[v]);out.writeInt(g.normalZ[v]);out.writeInt(g.normalMagnitude[v]);}}
                    if(loc)for(int i=0;i<m.triangleCount;i++){int index=lookup[i];if(index>=0){out.writeShort(g.triangleColors[index]);out.writeByte(g.triangleAlpha[index]);}}
                    if(p[0].equals("character")){bytes.reset();CharacterShadowOracle.animation=a;SeqType seq=new SeqType();seq.frames=new int[]{0};g=CharacterShadowOracle.getOrCreateShadow(160,Boolean.parseBoolean(p[11]),seq,Integer.parseInt(p[12]),0,Integer.parseInt(p[13]),0,1,g,Integer.parseInt(p[10]),0,0,240);for(int i=0;i<g.vertexCount;i++){out.writeInt(g.vertexX[i]);out.writeInt(g.vertexY[i]);out.writeInt(g.vertexZ[i]);}for(int i=0;i<g.triangleCount;i++){out.writeShort(g.triangleColors[i]);out.writeByte(g.triangleAlpha[i]);}}
                }else{
                    PoseModel pose=new PoseModel(m);pose.applyAnimationBlend(base,a,b,Integer.parseInt(p[6]),Integer.parseInt(p[7]),null,false,false,65535,null);canonical(new DataOutputStream(bytes),m);
                }
                if(p[8].equals(System.getProperty("visual.dumpPose")))Files.write(dir.resolve(p[8]+".pose"),bytes.toByteArray());
                System.out.println("pose\t"+p[8]+"\t"+digest(bytes.toByteArray()));
            } else if(p[0].equals("height")) {
                int x=Integer.parseInt(p[2]),y=Integer.parseInt(p[3]);
                System.out.println("height\t"+p[1]+"\t"+PerlinNoise.getTileHeight(x,y));
            } else if(p[0].equals("tile")) {
                ShapedTile tile=new ShapedTile(Integer.parseInt(p[2]),Integer.parseInt(p[3]),7,0,0,-16,-32,-96,-48,1,2,3,4,5,6,7,8,0,0);
                ByteArrayOutputStream bytes=new ByteArrayOutputStream();DataOutputStream out=new DataOutputStream(bytes);
                for(int i=0;i<tile.vertexX.length;i++){out.writeInt(tile.vertexX[i]);out.writeInt(tile.vertexY[i]);out.writeInt(tile.vertexZ[i]);}
                for(int i=0;i<tile.triangleVertexA.length;i++){out.writeByte(tile.triangleTextureId[i]!=-1?1:0);out.writeInt(tile.triangleVertexA[i]);out.writeInt(tile.triangleVertexB[i]);out.writeInt(tile.triangleVertexC[i]);}
                System.out.println("tile\t"+p[4]+"\t"+digest(bytes.toByteArray()));
            } else if(p[0].equals("sprite")) {
                SpriteOracle.decode(data);ByteArrayOutputStream bytes=new ByteArrayOutputStream();DataOutputStream out=new DataOutputStream(bytes);
                out.writeInt(SpriteOracle.width);out.writeInt(SpriteOracle.height);
                byte[] rgba=new byte[SpriteOracle.width*SpriteOracle.height*4];
                for(int y=0;y<SpriteOracle.innerHeights[0];y++)for(int x=0;x<SpriteOracle.innerWidths[0];x++){
                    int src=y*SpriteOracle.innerWidths[0]+x;int index=SpriteOracle.pixels[0][src]&255;int color=SpriteOracle.palette[index];int off=((y+SpriteOracle.yOffsets[0])*SpriteOracle.width+x+SpriteOracle.xOffsets[0])*4;
                    rgba[off]=(byte)(color>>16);rgba[off+1]=(byte)(color>>8);rgba[off+2]=(byte)color;rgba[off+3]=SpriteOracle.hasAlpha[0]?SpriteOracle.alpha[0][src]:(byte)(index==0?0:255);
                }
                out.write(rgba);System.out.println("sprite\t"+p[1]+"\t"+digest(bytes.toByteArray()));
            }
        }
    }
}
