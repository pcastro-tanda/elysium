X = T.type_alias { T.any(A, B) }
X0 = T.type_alias { X }
Constant = T.type_alias { Foo }
ConstantName = T.type_alias { T.any(A, B) }
HTTP = T.type_alias { Foo }
PARENT_NAME::ConstantName = T.type_alias { Foo }
