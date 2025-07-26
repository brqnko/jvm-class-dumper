package io.github.brqnko.retransformer;

import io.github.brqnko.bytekin.data.Injection;
import io.github.brqnko.bytekin.injection.At;
import io.github.brqnko.bytekin.transformer.BytekinTransformer;

public class Retransformer {

    private static final int ASM9 = 9 << 16;

    // this is called by rust side
    public static byte[] retransform(byte[] bytes, String className) {
        BytekinTransformer transformer = new BytekinTransformer.Builder()
                // hook EntityPlayerSP#onUpdate, io.github.brqnko.client.Client#onUpdate will be called
                // bew is EntityPlayerSP
                // t_ is onUpdate
                // for the mappping, refer to the mappings.tiny
                .inject(
                        "bew",
                        new Injection(
                                "t_",
                                "()V",
                                At.HEAD,
                                "io.github.brqnko.client.Client",
                                "onUpdate"
                        )
                )
                .build();

        return transformer.transform(className, bytes, ASM9);
    }

}
