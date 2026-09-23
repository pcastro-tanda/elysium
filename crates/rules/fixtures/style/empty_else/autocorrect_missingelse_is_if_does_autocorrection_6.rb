def foo
  if @params
    case @params[:x]
    when :a
      :b
    else
    ^^^^ Redundant `else`-clause.
      nil
    end
  else
    :c
  end
end
