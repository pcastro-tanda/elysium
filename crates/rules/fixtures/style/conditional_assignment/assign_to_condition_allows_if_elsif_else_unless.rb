if var.any?(:prob_a_check)
  @errors << 'Problem A'
elsif var.any?(:prob_a_check)
  @errors << 'Problem B'
else
  unless var.all?(:save)
    @errors << 'Save failed'
  end
end
