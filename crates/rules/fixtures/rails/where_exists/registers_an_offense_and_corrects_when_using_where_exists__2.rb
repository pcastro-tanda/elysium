User.where(name: 'john')&.exists?
     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `exists?(name: 'john')` over `where(name: 'john')&.exists?`.
