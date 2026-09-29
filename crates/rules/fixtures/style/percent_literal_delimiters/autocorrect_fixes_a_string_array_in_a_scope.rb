module Foo
   class Bar
     def baz
       %(one two)
       ^^^^^^^^^^ `%`-literals should be delimited by `[` and `]`.
     end
   end
 end
