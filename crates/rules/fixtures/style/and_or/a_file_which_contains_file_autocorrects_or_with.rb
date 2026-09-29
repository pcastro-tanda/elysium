APP_ROOT = Pathname.new File.expand_path('../../', __FILE__)
system('bundle check') or system!('bundle install')
                       ^^ Use `||` instead of `or`.
