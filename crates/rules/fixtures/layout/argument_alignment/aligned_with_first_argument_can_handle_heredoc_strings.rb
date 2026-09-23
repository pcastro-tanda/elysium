class_eval(<<-EOS, __FILE__, __LINE__ + 1)
            def run_#{name}_callbacks(*args)
              a = 1
              return value
            end
            EOS
