def method_name
^^^^^^^^^^^^^^^ Cyclomatic complexity for `method_name` is too high. [5/2]
  if first_condition then
    call_foo
  else
    if second_condition then
      call_bar
    else
      call_bam if third_condition
    end
    call_baz if fourth_condition
  end
end
