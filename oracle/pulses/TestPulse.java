package oracle;
import core.game.node.Node;
import core.game.system.task.Pulse;
import org.json.simple.JSONObject;
public class TestPulse extends Pulse {
    public TestPulse(){super();}public TestPulse(int delay){super(delay);}public TestPulse(int delay,Node[] checks){super(delay,checks);}
    @Override public boolean pulse(){PulseOracle.pulseHook(this,"pulse");return PulseOracle.flag(PulseOracle.config(this),"finish",false);}
    @Override public boolean update(){PulseOracle.pulseHook(this,"update");JSONObject d=PulseOracle.config(this);return d.get("updateResult") instanceof Boolean?(boolean)d.get("updateResult"):super.update();}
    @Override public boolean hasInactiveNode(){PulseOracle.pulseHook(this,"inactive");JSONObject d=PulseOracle.config(this);return d.get("inactiveResult") instanceof Boolean?(boolean)d.get("inactiveResult"):super.hasInactiveNode();}
    @Override public boolean isRunning(){PulseOracle.pulseHook(this,"isRunning");JSONObject d=PulseOracle.config(this);return d.get("runningResult") instanceof Boolean?(boolean)d.get("runningResult"):super.isRunning();}
    @Override public void start(){PulseOracle.pulseHook(this,"start");if(PulseOracle.flag(PulseOracle.config(this),"startBase",true))super.start();}
    @Override public void stop(){PulseOracle.pulseHook(this,"stop");if(PulseOracle.flag(PulseOracle.config(this),"stopBase",true))super.stop();}
    @Override public boolean removeFor(String value){PulseOracle.pulseHook(this,"removeFor",value);return PulseOracle.flag(PulseOracle.config(this),"removeResult",true);}
}
