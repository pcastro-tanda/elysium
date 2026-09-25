def parse_options
  index = -1
  while loop_cond
    index += 1

    if first_cond
      index += 1
    else
      if second_cond
        index += 1
      else
        if third_cond
          index += 1
        end
      end
    end
  end
end
