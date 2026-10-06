A = T.any(String, Integer)
    ^^^^^^^^^^^^^^^^^^^^^^ Sorbet/BindingConstantWithoutTypeAlias: It looks like you're trying to bind a type to a constant. To do this, you must alias the type using `T.type_alias`.
B = T.all(String, Integer)
    ^^^^^^^^^^^^^^^^^^^^^^ Sorbet/BindingConstantWithoutTypeAlias: It looks like you're trying to bind a type to a constant. To do this, you must alias the type using `T.type_alias`.
C = T.noreturn
    ^^^^^^^^^^ Sorbet/BindingConstantWithoutTypeAlias: It looks like you're trying to bind a type to a constant. To do this, you must alias the type using `T.type_alias`.
D = T.class_of(String)
    ^^^^^^^^^^^^^^^^^^ Sorbet/BindingConstantWithoutTypeAlias: It looks like you're trying to bind a type to a constant. To do this, you must alias the type using `T.type_alias`.
E = T.proc.void
    ^^^^^^^^^^^ Sorbet/BindingConstantWithoutTypeAlias: It looks like you're trying to bind a type to a constant. To do this, you must alias the type using `T.type_alias`.
F = T.untyped
    ^^^^^^^^^ Sorbet/BindingConstantWithoutTypeAlias: It looks like you're trying to bind a type to a constant. To do this, you must alias the type using `T.type_alias`.
G = T.nilable(String)
    ^^^^^^^^^^^^^^^^^ Sorbet/BindingConstantWithoutTypeAlias: It looks like you're trying to bind a type to a constant. To do this, you must alias the type using `T.type_alias`.
H = T.self_type
    ^^^^^^^^^^^ Sorbet/BindingConstantWithoutTypeAlias: It looks like you're trying to bind a type to a constant. To do this, you must alias the type using `T.type_alias`.
