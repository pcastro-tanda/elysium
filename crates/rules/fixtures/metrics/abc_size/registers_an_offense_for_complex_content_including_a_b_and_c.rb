def method_name
^^^^^^^^^^^^^^^ Assignment Branch Condition size for `method_name` is too high. [<3, 4, 5> 7.07/0]
  my_options = Hash.new if 1 == 1 || 2 == 2 # 1, 1, 4
  my_options.each do |key, value|           # 2, 1, 1
    p key                                   # 0, 1, 0
    p value                                 # 0, 1, 0
  end
end
