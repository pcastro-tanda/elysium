def batch
  @areas = params[:param].map {
               var_1 = 123_456
               variable_2 = 456_123 }
                          ^ `=` is not aligned with the preceding assignment.
  @another = params[:param].map {
           ^ `=` is not aligned with the preceding assignment.
               char_1 = begin
                          variable_1_1     = 'a'
                          variable_1_20  = 'b'
                                         ^ `=` is not aligned with the preceding assignment.

                          variable_1_300    = 'c'
                          # A Comment
                          variable_1_4000      = 'd'
                                               ^ `=` is not aligned with the preceding assignment.

                          variable_1_50000     = 'e'
                          puts 'a non-assignment statement without a blank line'
                          some_other_length_variable     = 'f'
                                                         ^ `=` is not aligned with the preceding assignment.
                        end
               var_2 = 456_123 }
                     ^ `=` is not aligned with the preceding assignment.

  render json: @areas
end
