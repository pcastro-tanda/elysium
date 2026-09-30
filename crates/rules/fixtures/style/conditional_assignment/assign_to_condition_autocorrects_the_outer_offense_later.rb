if var.any?(:prob_a_check)
  @errors << 'Problem A'
elsif var.any?(:prob_a_check)
  @errors << 'Problem B'
else
  if var.all?(:save)
  ^^^^^^^^^^^^^^^^^^ Use the return of the conditional for variable assignment and comparison.
    @errors << 'Save failed'
  else
    @errors << 'Other'
  end
end
