import core.game.node.entity.Entity;
import core.game.node.entity.skill.Skills;
import org.json.simple.*;
import org.json.simple.parser.JSONParser;
import java.io.*;
import java.util.*;
public class SkillBenchmark {
 static volatile long consumed;
 static long compute(Skills skills,double[] queries,int passes){long checksum=0;for(int pass=0;pass<passes;pass++)for(double query:queries)checksum=checksum*31+skills.levelFromXP(query);return checksum;}
 public static void main(String[] args)throws Exception{
  JSONObject input=(JSONObject)new JSONParser().parse(new FileReader(args[0]));List<?> raw=(List<?>)input.get("queries");double[] queries=new double[raw.size()];for(int i=0;i<queries.length;i++)queries[i]=Double.longBitsToDouble(Long.parseUnsignedLong((String)raw.get(i),16));
  oracle.Model.config=new JSONObject();Entity owner=new Entity();Skills skills=new Skills(owner);owner.skills=skills;
  int warmups=((Number)input.get("warmupPasses")).intValue(),passes=((Number)input.get("passes")).intValue(),repetitions=((Number)input.get("repetitions")).intValue();consumed=compute(skills,queries,warmups);
  List<Object> samples=new ArrayList<>();long checksum=0;for(int repeat=0;repeat<repetitions;repeat++){long start=System.nanoTime();checksum=compute(skills,queries,passes);samples.add(System.nanoTime()-start);consumed=checksum;}
  JSONObject out=new JSONObject();out.put("samplesNs",samples);out.put("checksum",String.format("%016x",checksum));out.put("javaVersion",System.getProperty("java.runtime.version"));out.put("vm",System.getProperty("java.vm.name"));System.out.println(out.toJSONString());
 }
}
