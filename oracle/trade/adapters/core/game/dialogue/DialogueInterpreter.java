package core.game.dialogue;

import core.game.node.entity.player.Player;

/** Recording interpreter: close() and the three input prompts. */
public class DialogueInterpreter {
	private final Player player;

	public DialogueInterpreter(Player player) {
		this.player = player;
	}

	public boolean close() {
		oracle.Support.emit("dialogueClose", oracle.Support.index(player));
		return true;
	}

	public void sendInput(boolean string, String prompt) {
		oracle.Support.emit("sendInput", oracle.Support.index(player), string, prompt);
	}

	public void sendLongInput(String prompt) {
		oracle.Support.emit("sendLongInput", oracle.Support.index(player), prompt);
	}

	public void sendMessageInput(String prompt) {
		oracle.Support.emit("sendMessageInput", oracle.Support.index(player), prompt);
	}
}