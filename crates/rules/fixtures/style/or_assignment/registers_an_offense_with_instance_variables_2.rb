@foo = if @foo
^^^^^^^^^^^^^^ Use the double pipe equals operator `||=` instead.
         @foo
       else
         'default'
       end
