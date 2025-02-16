resize:
	mkdir -p ../../client/paintings
	mogrify -resize 512x512 -format webp -quality 75 -path ../../client/data/paintings *.png
	rm ../../client/data/paintings/Runway.webp
	#optipng -o7 ../../client/data/paintings/*.png
