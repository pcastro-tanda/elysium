A = T.type_alias { T.any(String, Integer) }
B = T.type_alias { T.all(String, Integer) }
C = T.type_alias { T.noreturn }
D = T.type_alias { T.class_of(String) }
E = T.type_alias { T.proc.void }
F = T.type_alias { T.untyped }
G = T.type_alias { T.nilable(String) }
H = T.type_alias { T.self_type }
