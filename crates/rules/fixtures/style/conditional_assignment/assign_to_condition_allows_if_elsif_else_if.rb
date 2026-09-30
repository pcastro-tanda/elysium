if var.any?(:prob_a_check)
  @errors << 'Problem A'
elsif var.any?(:prob_a_check)
  @errors << 'Problem B'
else
  if var.all?(:save)
    @errors << 'Save failed'
  end
end
