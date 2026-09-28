Dir.glob(Rails.root.join('test', '*.rb'), sort: false).each(&method(:require))
