if var.any?(:prob_a_check)
  @errors << 'Problem A'
elsif var.any?(:prob_a_check)
  @errors << 'Problem B'
else
  @errors << if var.all?(:save)
    'Save failed'
  else
    'Other'
  end
end
