class ObjStack {int type,amount,serial;ObjStack(){serial=GroundItemsOracle.items.size()+1;GroundItemsOracle.items.add(this);}}
class ObjType {int cost=1,stackable;}
class ObjTypeList {
    static java.util.Map<Integer,ObjType> types=new java.util.HashMap<>();
    static ObjType get(int id){GroundItemsOracle.remember();GroundItemsOracle.events.add(java.util.List.of("info",id));GroundItemsOracle.hook("info");return types.computeIfAbsent(id,k->new ObjType());}
}
class Player {static int plane;}
class PlayerList {static int selfId;}
class SceneGraph {
    static int currentChunkX,currentChunkY;
    static LinkedList[][][] objStacks=new LinkedList[4][104][104];
    static java.util.Map<Integer,Pile> piles=new java.util.TreeMap<>();
    static class Pile {int height;long key;ObjStack a,b,c;Pile(int h,long k,ObjStack a,ObjStack b,ObjStack c){height=h;key=k;this.a=a;this.b=b;this.c=c;}}
    static int index(int p,int x,int y){if(p<0||p>=4||x<0||x>=104||y<0||y>=104)throw new ArrayIndexOutOfBoundsException();return(p*104+x)*104+y;}
    static int getTileHeight(int p,int x,int y){GroundItemsOracle.events.add(java.util.List.of("height",p,x,y));GroundItemsOracle.hook("height");return p*1000+x-y;}
    static void removeObjStack(int p,int x,int y){GroundItemsOracle.events.add(java.util.List.of("publish",p,x,y));GroundItemsOracle.hook("publish");piles.remove(index(p,x,y));}
    static void setObjStack(int p,int x,int y,int h,ObjStack a,long k,ObjStack b,ObjStack c){GroundItemsOracle.events.add(java.util.List.of("publish",p,x,y));GroundItemsOracle.hook("publish");piles.put(index(p,x,y),new Pile(h,k,a,b,c));}
}
public class GroundItemsOracle {
    static java.util.List<ObjStack>items;
    static java.util.List<ObjStackNode>nodes;
    static java.util.List<Object>events;
    static String hookStage,hookCommand;static int hookRemaining;
    static void hook(String stage){if(stage.equals(hookStage)&&--hookRemaining==0){String command=hookCommand;hookStage=null;command(command);}}
    static int n(String[]v,int i){return Integer.parseInt(v[i]);}
    static LinkedList list(int p,int x,int y){LinkedList l=SceneGraph.objStacks[p][x][y];if(l==null)SceneGraph.objStacks[p][x][y]=l=new LinkedList();return l;}
    static void remember(){while(nodes.size()<items.size())nodes.add(null);for(var plane:SceneGraph.objStacks)for(var row:plane)for(LinkedList l:row)if(l!=null)for(Node node=l.sentinel.nextNode;node!=l.sentinel;node=node.nextNode){ObjStackNode n=(ObjStackNode)node;nodes.set(n.value.serial-1,n);}}
    static void command(String line){String[]v=line.split(" ");switch(v[0]){
        case "M":{ObjType t=new ObjType();t.cost=n(v,2);t.stackable=n(v,3);ObjTypeList.types.put(n(v,1),t);break;}
        case "P":Player.plane=n(v,1);PlayerList.selfId=n(v,2);break;
        case "K":{ObjStack o=new ObjStack();o.type=n(v,4);o.amount=n(v,5);ObjStackNode node=new ObjStackNode(o);nodes.add(node);list(n(v,1),n(v,2),n(v,3)).addTail(node);break;}
        case "V":{ObjStack o=items.get(n(v,1)-1);o.type=n(v,2);o.amount=n(v,3);break;}
        case "D":nodes.get(n(v,1)-1).unlink();break;
        case "T":list(n(v,1),n(v,2),n(v,3)).addTail(nodes.get(n(v,4)-1));break;
        case "L":SceneGraph.objStacks[n(v,1)][n(v,2)][n(v,3)]=null;break;
        case "A":SceneGraph.objStacks[n(v,1)][n(v,2)][n(v,3)]=SceneGraph.objStacks[n(v,4)][n(v,5)][n(v,6)];break;
        case "E":SceneGraph.objStacks[n(v,1)][n(v,2)][n(v,3)]=new LinkedList();break;
        case "H":case "N":{LinkedList l=SceneGraph.objStacks[n(v,1)][n(v,2)][n(v,3)];ObjStackNode node=(ObjStackNode)(v[0].equals("H")?l.head():l.next());events.add(java.util.Arrays.asList("cursor",node==null?null:node.value.serial));break;}
        case "F":hookStage=v[1];hookRemaining=n(v,2);hookCommand=line.substring(line.indexOf(v[2])+v[2].length()+1);break;
        case "X":throw new IllegalStateException("callback failed");
        case "R":Protocol.spawnGroundObject(n(v,2),n(v,1));break;
        case "Z":GroundRebase.shift(n(v,1),n(v,2));break;
        case "U":{try{Protocol.frame(n(v,1),java.util.HexFormat.of().parseHex(v.length>2?v[2]:""));}finally{remember();}break;}
        default:throw new IllegalArgumentException(line);
    }}
    static Object item(ObjStack o){return o==null?null:java.util.List.of(o.serial,o.type,o.amount);}
    static Object snapshot(String error){var stacks=new java.util.ArrayList<>();for(int p=0;p<4;p++)for(int x=0;x<104;x++)for(int y=0;y<104;y++){LinkedList l=SceneGraph.objStacks[p][x][y];if(l!=null){var rows=new java.util.ArrayList<>();for(Node n=l.sentinel.nextNode;n!=l.sentinel;n=n.nextNode)rows.add(item(((ObjStackNode)n).value));stacks.add(java.util.List.of(p,x,y,rows));}}
        var piles=new java.util.ArrayList<>();for(var e:SceneGraph.piles.entrySet()){int k=e.getKey();var t=e.getValue();piles.add(java.util.Arrays.asList(k/(104*104),k/104%104,k%104,t.height,t.key,item(t.a),item(t.b),item(t.c)));}
        return java.util.Arrays.asList(error,Player.plane,PlayerList.selfId,SceneGraph.currentChunkX,SceneGraph.currentChunkY,stacks,piles,new java.util.ArrayList<>(events));
    }
    static String json(Object o){if(o==null)return"null";if(o instanceof Number)return o.toString();if(o instanceof String)return"\""+o+"\"";var b=new StringBuilder("[");for(Object v:(java.util.List<?>)o){if(b.length()>1)b.append(',');b.append(json(v));}return b.append(']').toString();}
    public static void main(String[]args)throws Exception{var input=new java.io.BufferedReader(new java.io.InputStreamReader(System.in,java.nio.charset.StandardCharsets.UTF_8));String line;while((line=input.readLine())!=null){SceneGraph.objStacks=new LinkedList[4][104][104];SceneGraph.piles.clear();SceneGraph.currentChunkX=SceneGraph.currentChunkY=Player.plane=PlayerList.selfId=0;ObjTypeList.types.clear();items=new java.util.ArrayList<>();nodes=new java.util.ArrayList<>();hookStage=null;var steps=new java.util.ArrayList<>();for(String command:line.split("\\|")){events=new java.util.ArrayList<>();String error=null;try{command(command);}catch(ArrayIndexOutOfBoundsException ex){error="bounds";}catch(NullPointerException ex){error="null";}catch(IllegalStateException ex){error="callback";}steps.add(snapshot(error));}System.out.println(json(steps));}}
}
