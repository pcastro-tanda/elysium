output = case variable
         in pattern
           'output1'
         else
           'output2'
         end
output = case variable
in pattern
^^ Indent `in` as deep as `case`.
  'output1'
else
  'output2'
end
