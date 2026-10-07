A = type_template(fixed: T.untyped)
A = type_template { { fixed: T.class_of(::ActiveRecord::Base) } }
A = type_template(:in) { { fixed: T.untyped } }
