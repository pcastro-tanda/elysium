var1 = nil
array_list = []
if var1.attr1 != 0 || array_list.select{ |w|
                        (w.attr2 == var1.attr2)
                 ^^^^^^^ Use 2 (not 7) spaces for indentation.
                 }.blank?
  array_list << var1
end
