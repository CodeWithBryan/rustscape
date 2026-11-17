package oracle;
import java.util.*;import java.nio.file.*;import java.nio.charset.StandardCharsets;
import core.game.system.config.GroundSpawnLoader;
import core.game.system.config.GroundSpawnLoader.GroundSpawn;
public class LoaderSupport {
    static GroundSpawnLoader loader;public static String path;static Path folder;
    static void reset(){loader=new GroundSpawnLoader();folder=Paths.get("runs/ground-spawn-loader-oracle/config").toAbsolutePath();try{Files.createDirectories(folder);}catch(Exception e){throw new RuntimeException(e);}path=folder.toString()+"/";}
    public static String category(String c){if(c.endsWith("Support$Failure"))return"Failure";if(c.contains("NullPointer"))return"Null";if(c.contains("IndexOutOfBounds"))return"Index";if(c.contains("NumberFormat"))return"Number";if(c.contains("ClassCast")||c.contains("TypeCast"))return"Type";if(c.contains("ParseException"))return"Json";if(c.startsWith("java.io."))return"Io";return c.substring(c.lastIndexOf('.')+1);}
    public static void log(int level,String message){if(level==3){int n=Integer.parseInt(message.substring("Initialized ".length(),message.indexOf(" ground items.")));Support.emit("loaderCount",n);return;}if(message.startsWith("Error parsing config entry ")){String line=message.substring("Error parsing config entry ".length()).split("\n",2)[0].replace("\r","");var matcher=java.util.regex.Pattern.compile("^(.*): ((?:java|kotlin|oracle)\\.[\\w.$]+)(?:: .*)?$").matcher(line);if(!matcher.matches())throw new AssertionError(line);Support.emit("loaderError",matcher.group(1),category(matcher.group(2)));return;}Support.emit("log",level,message);}
    static Object act(Map a){try{switch((String)a.get("op")){
        case"loaderLoad":{if(Boolean.TRUE.equals(a.get("missing"))){path=folder.toString()+"/missing-";}else{path=folder.toString()+"/";String text=Boolean.TRUE.equals(a.get("pinned"))?Files.readString(Paths.get("fixtures/ground-spawn-loader/ground_spawns.json"),StandardCharsets.UTF_8):(String)a.get("text");Files.writeString(folder.resolve("ground_spawns.json"),text,StandardCharsets.UTF_8);}loader.load();return null;}
        case"loaderRead":return loader.getReader()==null?null:loader.getReader().read();
        case"loaderStrings":{List<String>out=new ArrayList<>();for(var g:Support.grounds)if(g instanceof GroundSpawn)out.add(g.toString());return out;}
        default:throw new IllegalArgumentException((String)a.get("op"));
    }}catch(java.io.IOException e){throw new RuntimeException(e);}}
    static void snapshot(Map state){state.put("loaderReader",loader.getReader()!=null);List<Object>out=new ArrayList<>();for(var g:Support.grounds)if(g instanceof GroundSpawn){var s=(GroundSpawn)g;out.add(Arrays.asList(Support.gid(s),s.getRespawnRate(),s.toString()));}state.put("loadedSpawns",out);}
}
