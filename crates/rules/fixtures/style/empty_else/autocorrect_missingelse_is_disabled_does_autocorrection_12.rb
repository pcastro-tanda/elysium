foobar = case v
         when a
           foo
         when b
           bar
         else
         ^^^^ Redundant `else`-clause.
           nil
         end
