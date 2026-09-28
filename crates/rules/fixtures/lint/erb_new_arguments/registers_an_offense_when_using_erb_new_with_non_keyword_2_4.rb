ERB.new(str, nil, '-', '@output_buffer', trim_mode: '-', eoutvar: '@output_buffer')
                       ^^^^^^^^^^^^^^^^ Passing eoutvar with the 4th argument of `ERB.new` is deprecated. Use keyword argument like `ERB.new(str, eoutvar: '@output_buffer')` instead.
                  ^^^ Passing trim_mode with the 3rd argument of `ERB.new` is deprecated. Use keyword argument like `ERB.new(str, trim_mode: '-')` instead.
             ^^^ Passing safe_level with the 2nd argument of `ERB.new` is deprecated. Do not use it, and specify other arguments as keyword arguments.
