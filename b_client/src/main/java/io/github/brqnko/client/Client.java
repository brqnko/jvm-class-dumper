package io.github.brqnko.client;

import io.github.brqnko.bytekin.injection.CallbackInfo;
import net.minecraft.client.entity.EntityPlayerSP;

public class Client {

    /**
     * This method is called when EntityPlayerSP#onUpdate is invoked.
     *
     * @param player self reference to the player entity
     * @return CallbackInfo instance, can be used to cancel the method or modify its behavior
     */
    public static CallbackInfo onUpdate(EntityPlayerSP player) {
        player.setHealth(20);
        return CallbackInfo.empty();
    }

}
