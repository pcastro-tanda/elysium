var1 = nil
array_list = []
if var1.attr1 != 0 || array_list.select{ |w|
                   (w.attr2 == var1.attr2)
                 }.blank?
  array_list << var1
end
