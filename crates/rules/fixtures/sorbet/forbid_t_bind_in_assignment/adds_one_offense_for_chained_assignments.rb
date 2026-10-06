foo = bar = T.bind(self, Integer)
            ^^^^^^^^^^^^^^^^^^^^^ Sorbet/ForbidTBindInAssignment: Do not assign the result of `T.bind`; it also changes the type of its first argument.
