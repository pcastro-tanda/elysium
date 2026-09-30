output = case variable
         in pattern
         ^^ Indent `in` one step more than `case`.
           'output1'
         else
           'output2'
         end
