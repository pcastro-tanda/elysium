def batch
  @areas   = params[:param].map do
               var_1      = 123_456
               variable_2 = 456_123
             end
  @another = params[:param].map do
               char_1 = begin
                          variable_1_1  = 'a'
                          variable_1_20 = 'b'

                          variable_1_300  = 'c'
                          # A Comment
                          variable_1_4000 = 'd'

                          variable_1_50000           = 'e'
                          puts 'a non-assignment statement without a blank line'
                          some_other_length_variable = 'f'
                        end
               var_2  = 456_123
             end

  render json: @areas
end
