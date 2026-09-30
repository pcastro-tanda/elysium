def method_name
^^^^^^^^^^^^^^^ Perceived complexity for `method_name` is too high. [6/1]
  if first_condition then
    call_foo if second_condition && third_condition
    call_bar if fourth_condition || fifth_condition
  end
end
