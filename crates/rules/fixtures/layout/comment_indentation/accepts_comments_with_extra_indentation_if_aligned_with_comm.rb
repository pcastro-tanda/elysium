def compile_sequence(seq, seq_var)
  @seq_var = seq_var # Holds the name of the variable holding the AST::Node we are matching

  context.with_temp_variables do |cur_child, cur_index, previous_index|
    @cur_child_var = cur_child        # To hold the current child node
    @cur_index_var = cur_index        # To hold the current child index (always >= 0)
    @prev_index_var = previous_index  # To hold the child index before we enter the looping nodes
    @cur_index = :seq_head            # Can be any of:
                                      #   :seq_head : when the current child is actually the sequence head
                                      #   :variadic_mode : child index held by @cur_index_var
                                      #   >= 0 : when the current child index is known (from the beginning)
                                      #   < 0 : when the index is known from the end, where -1 is *past the end*,
                                      #         -2 is the last child, etc...
                                      #         This shift of 1 from standard Ruby indices is stored in DELTA
    @in_sync = false                  # `true` iff `@cur_child_var` and `@cur_index_var` correspond to `@cur_index`
                                      # Must be true if `@cur_index` is `:variadic_mode`
    compile_terms(seq)
  end
end
