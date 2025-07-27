package io.github.brqnko.retransformer;

import org.objectweb.asm.*;

import java.util.Set;
import java.util.TreeSet;

public class Retransformer {

    // this is called by rust side
    public static String[] getAllDependencies(byte[] bytes) {
        Set<String> dependencies = new TreeSet<>();

        ClassReader reader = new ClassReader(bytes);

        reader.accept(new ClassVisitor(Opcodes.ASM9) {
            @Override
            public void visit(int version, int access, String name, String signature, String superName, String[] interfaces) {
                if (superName != null) {
                    addName(superName);
                }
                if (interfaces != null) {
                    for (String iface : interfaces) {
                        addName(iface);
                    }
                }
                super.visit(version, access, name, signature, superName, interfaces);
            }

            @Override
            public FieldVisitor visitField(int access, String name, String descriptor, String signature, Object value) {
                addDesc(descriptor);
                return super.visitField(access, name, descriptor, signature, value);
            }

            @Override
            public MethodVisitor visitMethod(int access, String name, String descriptor, String signature, String[] exceptions) {
                addMethodDesc(descriptor);
                if (exceptions != null) {
                    for (String exc : exceptions) {
                        addName(exc);
                    }
                }

                return new MethodVisitor(Opcodes.ASM9) {

                    @Override
                    public void visitTypeInsn(int opcode, String type) {
                        addName(type);
                        super.visitTypeInsn(opcode, type);
                    }

                    @Override
                    public void visitFieldInsn(int opcode, String owner, String name, String descriptor) {
                        addName(owner);
                        addDesc(descriptor);
                        super.visitFieldInsn(opcode, owner, name, descriptor);
                    }

                    @Override
                    public void visitMethodInsn(int opcode, String owner, String name, String descriptor, boolean isInterface) {
                        addName(owner);
                        addMethodDesc(descriptor);
                        super.visitMethodInsn(opcode, owner, name, descriptor, isInterface);
                    }

                    @Override
                    public void visitLdcInsn(Object value) {
                        if (value instanceof Type) {
                            addType((Type) value);
                        }
                        super.visitLdcInsn(value);
                    }

                    @Override
                    public void visitMultiANewArrayInsn(String descriptor, int dims) {
                        addDesc(descriptor);
                        super.visitMultiANewArrayInsn(descriptor, dims);
                    }

                    @Override
                    public void visitTryCatchBlock(Label start, Label end, Label handler, String type) {
                        if (type != null) {
                            addName(type);
                        }
                        super.visitTryCatchBlock(start, end, handler, type);
                    }
                };
            }

            private void addName(String name) {
                if (name == null || name.startsWith("[")) {
                    return;
                }
                dependencies.add(name.replace('/', '.'));
            }

            private void addDesc(String desc) {
                addType(Type.getType(desc));
            }

            private void addMethodDesc(String methodDesc) {
                addType(Type.getReturnType(methodDesc));
                for (Type type : Type.getArgumentTypes(methodDesc)) {
                    addType(type);
                }
            }

            private void addType(Type type) {
                switch (type.getSort()) {
                    case Type.ARRAY:
                        addType(type.getElementType());
                        break;
                    case Type.OBJECT:
                        addName(type.getInternalName());
                        break;
                }
            }

        }, ClassReader.SKIP_DEBUG | ClassReader.SKIP_FRAMES);

        dependencies.remove("java.lang.Object");

        return dependencies.toArray(new String[0]);
    }

}
