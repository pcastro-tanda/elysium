APP_ROOT = Pathname.new File.expand_path('../../', __FILE__)
system('bundle check') || system!('bundle install')
