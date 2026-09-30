def method_name
^^^^^^^^^^^^^^^ Perceived complexity for `method_name` is too high. [3/1]
  case value
  in Integer if value > 0 then call_foo_1
  in Integer if value < 0 then call_foo_2
  in Integer then call_foo_3
  end
end
