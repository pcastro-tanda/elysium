Dir.glob(Rails.root.join('test', '*.rb')).each(&method(:require))
