package oracle;
import java.io.*;
import java.lang.reflect.*;
import java.text.*;
import java.util.*;
import org.json.simple.*;
import org.json.simple.parser.JSONParser;

public class CaseOracle {
    static Object field(Object owner,String key)throws Exception {Field f=owner.getClass().getDeclaredField(key);f.setAccessible(true);return f.get(owner);}
    static Object data()throws Exception {
        BreakIterator word=BreakIterator.getWordInstance(Locale.ROOT);
        Method category=word.getClass().getDeclaredMethod("lookupCategory",int.class);category.setAccessible(true);
        Method cased=Class.forName("java.lang.ConditionalSpecialCasing").getDeclaredMethod("isCased",int.class);cased.setAccessible(true);
        ArrayList<Object> pairs=new ArrayList<>(),ranges=new ArrayList<>(),casedRanges=new ArrayList<>();
        int start=0,prev=-2,casedStart=-1;
        for(int c=0;c<=0x10ffff;c++) {
            int cat=(int)category.invoke(word,c);
            if(cat!=prev){if(c>0)ranges.add(Arrays.asList(start,c-1,prev));start=c;prev=cat;}
            boolean isCased=(boolean)cased.invoke(null,c);
            if(isCased&&casedStart<0)casedStart=c;
            if(!isCased&&casedStart>=0){casedRanges.add(Arrays.asList(casedStart,c-1));casedStart=-1;}
            if(c>=0xd800&&c<=0xdfff)continue;
            String s=new String(Character.toChars(c)),lower=s.toLowerCase(Locale.ROOT);
            if(!s.equals(lower))pairs.add(Arrays.asList(c,lower));
        }
        ranges.add(Arrays.asList(start,0x10ffff,prev));if(casedStart>=0)casedRanges.add(Arrays.asList(casedStart,0x10ffff));
        Map<String,Object> data=new TreeMap<>();data.put("lowerPairs",pairs);data.put("wordRanges",ranges);data.put("casedRanges",casedRanges);
        short[] state=(short[])field(word,"stateTable");ArrayList<Integer> states=new ArrayList<>();for(short s:state)states.add((int)s);
        boolean[] end=(boolean[])field(word,"endStates"),look=(boolean[])field(word,"lookaheadStates");ArrayList<Boolean> ends=new ArrayList<>(),looks=new ArrayList<>();for(boolean b:end)ends.add(b);for(boolean b:look)looks.add(b);
        short[] backwards=(short[])field(word,"backwardsStateTable");ArrayList<Integer> back=new ArrayList<>();for(short s:backwards)back.add((int)s);
        data.put("backwards",back);data.put("states",states);data.put("end",ends);data.put("lookahead",looks);data.put("categories",field(word,"numCategories"));return data;
    }
    public static void main(String[]args)throws Exception {
        if(args.length>0&&args[0].equals("data")){System.out.println(JSONValue.toJSONString(data()));return;}
        BufferedReader in=new BufferedReader(new InputStreamReader(System.in,java.nio.charset.StandardCharsets.UTF_8));String line;
        while((line=in.readLine())!=null){JSONObject o=(JSONObject)new JSONParser().parse(line);String text=(String)o.get("text");o.put("lower",text.toLowerCase(Locale.ROOT));BreakIterator b=BreakIterator.getWordInstance(Locale.ROOT);b.setText(text);ArrayList<Integer> boundaries=new ArrayList<>();for(int n=b.first();n!=BreakIterator.DONE;n=b.next())boundaries.add(n);o.put("boundaries",boundaries);System.out.println(o.toJSONString());}
    }
}
