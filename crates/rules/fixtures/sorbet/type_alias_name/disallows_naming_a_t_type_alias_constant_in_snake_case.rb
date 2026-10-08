A_B = T.type_alias { T.any(A, B) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Sorbet/TypeAliasName: Type alias constant name should be in CamelCase
A_ = T.type_alias { T.any(A, B) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Sorbet/TypeAliasName: Type alias constant name should be in CamelCase
A_0 = T.type_alias { T.any(A, B) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Sorbet/TypeAliasName: Type alias constant name should be in CamelCase
CONSTANT_NAME = T.type_alias { T.any(A, B) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Sorbet/TypeAliasName: Type alias constant name should be in CamelCase
PARENT::CONSTANT_NAME = T.type_alias { T.any(A, B) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Sorbet/TypeAliasName: Type alias constant name should be in CamelCase
