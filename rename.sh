find . -iname entities.trb | while read path; do cp $path /c/Users/ssaam/code/nickview/trb/${path/\/Entities/}; done;
