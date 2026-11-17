package com.moandjiezana.toml;

/** The server configuration read as a recorded service: getBoolean answers from the case. */
public class Toml {
	public Boolean getBoolean(String key, Boolean fallback) {
		oracle.Support.emit("config", key);
		Object value = oracle.Support.config(key);
		return value == null ? fallback : (Boolean) value;
	}
}
