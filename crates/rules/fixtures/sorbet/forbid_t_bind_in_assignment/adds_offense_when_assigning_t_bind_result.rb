foo = T.bind(self, Integer)
      ^^^^^^^^^^^^^^^^^^^^^ Sorbet/ForbidTBindInAssignment: Do not assign the result of `T.bind`; it also changes the type of its first argument.
@foo = T.bind(self, Integer)
       ^^^^^^^^^^^^^^^^^^^^^ Sorbet/ForbidTBindInAssignment: Do not assign the result of `T.bind`; it also changes the type of its first argument.
self.foo = T.bind(self, Integer)
           ^^^^^^^^^^^^^^^^^^^^^ Sorbet/ForbidTBindInAssignment: Do not assign the result of `T.bind`; it also changes the type of its first argument.
foo ||= T.bind(self, Integer)
        ^^^^^^^^^^^^^^^^^^^^^ Sorbet/ForbidTBindInAssignment: Do not assign the result of `T.bind`; it also changes the type of its first argument.
foo, bar = T.bind(self, Integer)
           ^^^^^^^^^^^^^^^^^^^^^ Sorbet/ForbidTBindInAssignment: Do not assign the result of `T.bind`; it also changes the type of its first argument.
