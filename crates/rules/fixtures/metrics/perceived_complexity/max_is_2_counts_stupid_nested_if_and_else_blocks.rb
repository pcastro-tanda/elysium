def method_name                   # 1
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Perceived complexity for `method_name` is too high. [7/2]
  if first_condition then         # 2
    call_foo
  else                            # 3
    if second_condition then      # 4
      call_bar
    else                          # 5
      call_bam if third_condition # 6
    end
    call_baz if fourth_condition  # 7
  end
end
