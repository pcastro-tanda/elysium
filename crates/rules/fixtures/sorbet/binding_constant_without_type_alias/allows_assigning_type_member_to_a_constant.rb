A = type_member(fixed: T.untyped)
A = type_member { { fixed: T.class_of(::ActiveRecord::Base) } }
A = type_member(:in) { { fixed: T.untyped } }
